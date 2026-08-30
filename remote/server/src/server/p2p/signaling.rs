//! In-memory signaling registry and WebSocket relays.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::extract::WebSocketUpgrade;
use axum::extract::ws;
use axum::http::StatusCode;
use axum::response::IntoResponse as _;
use axum::response::Response;
use futures::SinkExt as _;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tracing::Instrument as _;
use tracing::info_span;
use tracing::warn;
use trz_gateway_common::id::ClientName;
use trz_gateway_common::p2p::protocol::FailureCode;
use trz_gateway_common::p2p::protocol::MAX_SDP_LEN;
use trz_gateway_common::p2p::protocol::PROTOCOL_VERSION;
use trz_gateway_common::p2p::protocol::SessionDescription;
use trz_gateway_common::p2p::protocol::SignalMessage;

use self::session::Session;

use self::registration::Registration;

mod registration;
mod session;

#[cfg(test)]
mod tests;

const REGISTRATION_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const SIGNAL_QUEUE_CAPACITY: usize = 128;
const MAX_SESSIONS_PER_SERVER: usize = 64;
const MAX_PENDING_SESSIONS: usize = 1024;
const MAX_SIGNAL_MESSAGE_SIZE: usize = MAX_SDP_LEN + 64 * 1024;

/// Coordinates server registrations and pending P2P signaling sessions.
///
/// One instance is owned by [`super::super::Server`]. Its maps contain only
/// signaling metadata; WebRTC media and application traffic never pass through
/// this component.
#[derive(Default)]
pub(in crate::server) struct Signaling {
    state: Mutex<State>,
    next_generation: AtomicU64,
    pending_sessions: Arc<AtomicUsize>,
    shutdown: watch::Sender<bool>,
}

/// Registry state updated atomically while holding [`Signaling::state`].
#[derive(Default)]
struct State {
    registrations: HashMap<ClientName, Arc<Registration>>,
    waiters: HashMap<ClientName, Waiters>,
}

/// Subscribers waiting for one server name to acquire an active registration.
///
/// `count` tracks live wait futures so the map entry can be removed promptly
/// when all requests complete or are cancelled.
struct Waiters {
    sender: watch::Sender<Option<Arc<Registration>>>,
    count: usize,
}

impl Default for Waiters {
    fn default() -> Self {
        let (sender, _) = watch::channel(None);
        Self { sender, count: 0 }
    }
}

impl Signaling {
    pub(super) fn register(
        self: Arc<Self>,
        server_name: ClientName,
        web_socket: WebSocketUpgrade,
    ) -> Response {
        web_socket
            .max_message_size(MAX_SIGNAL_MESSAGE_SIZE)
            .on_upgrade(move |socket| {
                let span = info_span!("P2pRegister", %server_name);
                self.serve_registration(server_name, socket)
                    .instrument(span)
            })
    }

    pub(super) async fn connect(
        self: Arc<Self>,
        server_name: ClientName,
        web_socket: WebSocketUpgrade,
    ) -> Response {
        let registration = match tokio::time::timeout(
            REGISTRATION_WAIT_TIMEOUT,
            self.wait_for_registration(server_name.clone()),
        )
        .await
        {
            Ok(Some(registration)) => registration,
            Ok(None) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
            Err(_) => return StatusCode::NOT_FOUND.into_response(),
        };
        let session = match registration.create_session() {
            Ok(session) => session,
            Err(()) => return StatusCode::TOO_MANY_REQUESTS.into_response(),
        };
        web_socket
            .max_message_size(MAX_SIGNAL_MESSAGE_SIZE)
            .on_upgrade(move |socket| {
                let span = info_span!(
                    "P2pConnect",
                    %server_name,
                    connection_id = %session.connection_id,
                    registration_generation = session.registration.generation,
                );
                self.serve_client(session, socket).instrument(span)
            })
    }

    fn install_registration(
        &self,
        server_name: ClientName,
        outgoing: mpsc::Sender<SignalMessage>,
    ) -> Arc<Registration> {
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let (close, _) = watch::channel(false);
        let registration = Arc::new(Registration::new(
            generation,
            outgoing,
            self.pending_sessions.clone(),
            close,
        ));
        let (previous, waiter) = {
            let mut state = self.state.lock().expect("signaling state");
            let previous = state
                .registrations
                .insert(server_name.clone(), registration.clone());
            let waiter = state
                .waiters
                .get(&server_name)
                .map(|waiters| waiters.sender.clone());
            (previous, waiter)
        };
        if let Some(previous) = previous {
            previous.cancel(
                FailureCode::PeerDisconnected,
                "Server registration was replaced",
            );
        }
        if let Some(waiter) = waiter {
            waiter.send_replace(Some(registration.clone()));
        }
        registration
    }

    fn remove_registration(&self, server_name: &ClientName, registration: &Arc<Registration>) {
        let removed = {
            let mut state = self.state.lock().expect("signaling state");
            match state.registrations.get(server_name) {
                Some(current) if Arc::ptr_eq(current, registration) => {
                    state.registrations.remove(server_name)
                }
                _ => None,
            }
        };
        if let Some(removed) = removed {
            removed.cancel(FailureCode::PeerDisconnected, "Server disconnected");
        }
    }

    async fn wait_for_registration(
        self: &Arc<Self>,
        server_name: ClientName,
    ) -> Option<Arc<Registration>> {
        let mut receiver = {
            let mut state = self.state.lock().expect("signaling state");
            if let Some(registration) = state.registrations.get(&server_name) {
                return Some(registration.clone());
            }
            let waiters = state.waiters.entry(server_name.clone()).or_default();
            waiters.count += 1;
            waiters.sender.subscribe()
        };
        let _guard = WaiterGuard {
            signaling: self.clone(),
            server_name,
        };
        let mut shutdown = self.shutdown.subscribe();
        loop {
            if let Some(registration) = receiver.borrow_and_update().clone() {
                return Some(registration);
            }
            tokio::select! {
                changed = receiver.changed() => changed.ok()?,
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        return None;
                    }
                }
            }
        }
    }

    async fn serve_registration(
        self: Arc<Self>,
        server_name: ClientName,
        mut socket: ws::WebSocket,
    ) {
        if receive_hello(&mut socket).await.is_err() {
            return;
        }
        let (outgoing, mut outgoing_rx) = mpsc::channel(SIGNAL_QUEUE_CAPACITY);
        let registration = self.install_registration(server_name.clone(), outgoing);
        let mut close = registration.close.subscribe();
        let mut shutdown = self.shutdown.subscribe();

        loop {
            tokio::select! {
                message = outgoing_rx.recv() => match message {
                    Some(message) if send_json(&mut socket, &message).await.is_ok() => {}
                    _ => break,
                },
                message = socket.recv() => match message {
                    Some(Ok(message)) => match parse_session_message(message) {
                        Ok(message) if valid_server_message(&message) => {
                            if registration.relay_to_client(message).is_err() {
                                break;
                            }
                        }
                        _ => break,
                    },
                    _ => break,
                },
                changed = close.changed() => {
                    if changed.is_err() || *close.borrow() {
                        break;
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
            }
        }
        self.remove_registration(&server_name, &registration);
        let _ = socket.close().await;
    }

    async fn serve_client(self: Arc<Self>, mut session: Session, mut socket: ws::WebSocket) {
        let connection_id = session.connection_id;
        if receive_hello(&mut socket).await.is_err()
            || send_json(&mut socket, &SignalMessage::Start { connection_id })
                .await
                .is_err()
            || session
                .registration
                .outgoing
                .send(SignalMessage::Start { connection_id })
                .await
                .is_err()
        {
            session.registration.remove_session(connection_id);
            return;
        }

        let mut shutdown = self.shutdown.subscribe();
        let timeout = tokio::time::sleep(HANDSHAKE_TIMEOUT);
        tokio::pin!(timeout);
        let mut notify_server = true;
        loop {
            tokio::select! {
                message = session.incoming.recv() => match message {
                    Some(message) => {
                        let done = matches!(
                            message,
                            SignalMessage::Cancel { .. } | SignalMessage::Failure { .. }
                        );
                        if send_json(&mut socket, &message).await.is_err() || done {
                            notify_server = false;
                            break;
                        }
                    }
                    None => {
                        notify_server = false;
                        break;
                    }
                },
                message = socket.recv() => match message {
                    Some(Ok(message)) => match parse_session_message(message) {
                        Ok(message)
                            if valid_client_message(&message)
                                && message.connection_id() == Some(connection_id) =>
                        {
                            let done = matches!(
                                message,
                                SignalMessage::Cancel { .. } | SignalMessage::Failure { .. }
                            );
                            if session.registration.outgoing.send(message).await.is_err() || done {
                                notify_server = false;
                                break;
                            }
                        }
                        _ => break,
                    },
                    _ => break,
                },
                () = &mut timeout => {
                    let failure = SignalMessage::Failure {
                        connection_id,
                        code: FailureCode::NegotiationFailed,
                        detail: "WebRTC handshake timed out; configure TURN when no direct ICE pair is viable".into(),
                    };
                    let _ = send_json(&mut socket, &failure).await;
                    let _ = session.registration.outgoing.send(failure).await;
                    notify_server = false;
                    break;
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
            }
        }
        session.registration.remove_session(connection_id);
        if notify_server {
            let _ = session
                .registration
                .outgoing
                .send(SignalMessage::Cancel { connection_id })
                .await;
        }
        let _ = socket.close().await;
    }

    pub(in crate::server) fn shutdown(&self) {
        self.shutdown.send_replace(true);
        let registrations = {
            let mut state = self.state.lock().expect("signaling state");
            state
                .registrations
                .drain()
                .map(|(_, value)| value)
                .collect::<Vec<_>>()
        };
        for registration in registrations {
            registration.cancel(FailureCode::PeerDisconnected, "Signaling server shut down");
        }
    }
}

/// Removes a registration waiter from the registry on completion or cancellation.
struct WaiterGuard {
    signaling: Arc<Signaling>,
    server_name: ClientName,
}

impl Drop for WaiterGuard {
    fn drop(&mut self) {
        let mut state = self.signaling.state.lock().expect("signaling state");
        let remove = if let Some(waiters) = state.waiters.get_mut(&self.server_name) {
            waiters.count -= 1;
            waiters.count == 0
        } else {
            false
        };
        if remove {
            state.waiters.remove(&self.server_name);
        }
    }
}

async fn receive_hello(socket: &mut ws::WebSocket) -> Result<(), ()> {
    let message = tokio::time::timeout(HANDSHAKE_TIMEOUT, socket.recv())
        .await
        .map_err(|_| ())?
        .ok_or(())?
        .map_err(|_| ())?;
    match parse_message(message)? {
        SignalMessage::Hello { protocol_version } if protocol_version == PROTOCOL_VERSION => Ok(()),
        _ => Err(()),
    }
}

fn parse_session_message(message: ws::Message) -> Result<SignalMessage, ()> {
    let message = parse_message(message)?;
    message.validate().map_err(|_| ())?;
    message.connection_id().ok_or(())?;
    Ok(message)
}

fn parse_message(message: ws::Message) -> Result<SignalMessage, ()> {
    let ws::Message::Text(text) = message else {
        return Err(());
    };
    serde_json::from_str(&text).map_err(|error| {
        warn!(%error, "Invalid P2P signaling message");
    })
}

async fn send_json(socket: &mut ws::WebSocket, message: &SignalMessage) -> Result<(), ()> {
    let json = serde_json::to_string(message).map_err(|_| ())?;
    socket
        .send(ws::Message::Text(json.into()))
        .await
        .map_err(|_| ())
}

/// Returns whether a registered server may send this message toward a client.
fn valid_server_message(message: &SignalMessage) -> bool {
    matches!(
        message,
        SignalMessage::Description {
            description: SessionDescription::Answer(_),
            ..
        } | SignalMessage::IceCandidate { .. }
            | SignalMessage::EndOfCandidates { .. }
            | SignalMessage::Cancel { .. }
            | SignalMessage::Failure { .. }
    )
}

/// Returns whether a connecting client may send this message toward a server.
fn valid_client_message(message: &SignalMessage) -> bool {
    matches!(
        message,
        SignalMessage::Description {
            description: SessionDescription::Offer(_),
            ..
        } | SignalMessage::IceCandidate { .. }
            | SignalMessage::EndOfCandidates { .. }
            | SignalMessage::Cancel { .. }
            | SignalMessage::Failure { .. }
    )
}
