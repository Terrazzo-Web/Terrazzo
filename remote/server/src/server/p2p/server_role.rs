//! Outbound signaling registration and WebRTC answer role for NATed servers.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use futures::FutureExt as _;
use futures::SinkExt as _;
use futures::StreamExt as _;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::rt::TokioTimer;
use hyper_util::server::conn::auto;
use hyper_util::service::TowerToHyperService;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tracing::Instrument as _;
use tracing::info;
use tracing::info_span;
use tracing::warn;
use trz_gateway_common::p2p::data_channel_io::DataChannelIo;
use trz_gateway_common::p2p::peer_connection::LocalIceEvent;
use trz_gateway_common::p2p::peer_connection::PeerConnectionBuilder;
use trz_gateway_common::p2p::protocol::FailureCode;
use trz_gateway_common::p2p::protocol::MAX_FAILURE_DETAIL_LEN;
use trz_gateway_common::p2p::protocol::MAX_SDP_LEN;
use trz_gateway_common::p2p::protocol::P2pConnectionId;
use trz_gateway_common::p2p::protocol::PROTOCOL_VERSION;
use trz_gateway_common::p2p::protocol::SessionDescription;
use trz_gateway_common::p2p::protocol::SignalMessage;
use url::Url;

use super::super::HTTP_TIMEOUT;
use super::super::Server;
use super::super::gateway_config::p2p::P2pRegistrationAuthorization;
use super::super::gateway_config::p2p::P2pRegistrationConfig;

const REGISTRATION_QUEUE_CAPACITY: usize = 128;
const SESSION_QUEUE_CAPACITY: usize = 64;
const ICE_QUEUE_CAPACITY: usize = 64;
const MAX_SIGNAL_MESSAGE_SIZE: usize = MAX_SDP_LEN + 64 * 1024;

// TODO: move this implem to remote/server/src/server/p2p/server/role.rs
impl Server {
    pub(in crate::server) fn start_p2p_registration(
        self: &Arc<Self>,
        config: P2pRegistrationConfig,
    ) {
        let server = self.clone();
        let server_name = config.server_name.clone();
        tokio::spawn(
            async move {
                let mut retry = config.retry_strategy.clone();
                loop {
                    let started = Instant::now();
                    let result = server.clone().run_p2p_registration(config.clone()).await;
                    if server.shutdown.clone().now_or_never().is_some() {
                        return;
                    }
                    match result {
                        Ok(()) => info!("P2P signaling registration closed"),
                        Err(error) => warn!(%error, "P2P signaling registration failed"),
                    }
                    if started.elapsed() >= config.retry_strategy.max_delay() {
                        retry = config.retry_strategy.clone();
                    }
                    let delay = retry.wait();
                    tokio::select! {
                        () = delay => {}
                        () = server.shutdown.clone() => return,
                    }
                }
            }
            .instrument(info_span!("P2pServer", %server_name)),
        );
    }

    async fn run_p2p_registration(
        self: Arc<Self>,
        config: P2pRegistrationConfig,
    ) -> Result<(), P2pServerError> {
        if config.max_sessions == 0 {
            return Err(P2pServerError::InvalidConfig(
                "max_sessions must be greater than zero".into(),
            ));
        }
        let request = registration_request(&config)?;
        let websocket_config = tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(MAX_SIGNAL_MESSAGE_SIZE))
            .max_frame_size(Some(MAX_SIGNAL_MESSAGE_SIZE));
        let (mut socket, _) =
            connect_async_with_config(request, Some(websocket_config), false).await?;
        send_signal(
            &mut socket,
            &SignalMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
            },
        )
        .await?;
        info!("Registered outbound signaling WebSocket");

        let semaphore = Arc::new(Semaphore::new(config.max_sessions));
        let (outgoing, mut outgoing_rx) = mpsc::channel(REGISTRATION_QUEUE_CAPACITY);
        let (done_tx, mut done_rx) = mpsc::channel(config.max_sessions);
        let mut sessions = HashMap::<P2pConnectionId, mpsc::Sender<SignalMessage>>::new();

        loop {
            // TODO: move the content of the loop to a separate method
            tokio::select! {
                outgoing = outgoing_rx.recv() => {
                    let Some(outgoing) = outgoing else {
                        return Ok(());
                    };
                    send_signal(&mut socket, &outgoing).await?;
                }
                incoming = socket.next() => {
                    let Some(incoming) = incoming else {
                        return Ok(());
                    };
                    let message = match incoming? {
                        Message::Text(text) => parse_signal_text(&text)?,
                        Message::Close(_) => return Ok(()),
                        Message::Ping(_) | Message::Pong(_) => {
                            socket.flush().await?;
                            continue;
                        }
                        Message::Binary(_) | Message::Frame(_) => {
                            return Err(P2pServerError::Protocol(
                                "Signaling messages must be JSON text".into(),
                            ));
                        }
                    };
                    // TODO: move the match block to a separate method
                    match message {
                        SignalMessage::Start { connection_id } => {
                            if sessions.contains_key(&connection_id) {
                                send_failure(
                                    &outgoing,
                                    connection_id,
                                    FailureCode::InvalidMessage,
                                    "Duplicate session start",
                                ).await;
                                continue;
                            }
                            let Ok(permit) = semaphore.clone().try_acquire_owned() else {
                                send_failure(
                                    &outgoing,
                                    connection_id,
                                    FailureCode::CapacityExceeded,
                                    "This server has too many active P2P sessions",
                                ).await;
                                continue;
                            };
                            let (session_tx, session_rx) = mpsc::channel(SESSION_QUEUE_CAPACITY);
                            sessions.insert(connection_id, session_tx);
                            let mut session = AnswerSession {
                                server: self.clone(),
                                config: config.clone(),
                                connection_id,
                                incoming: session_rx,
                                outgoing: outgoing.clone(),
                                permit: Some(permit),
                            };
                            let done_tx = done_tx.clone();
                            tokio::spawn(
                                async move {
                                    if let Err(error) = session.run().await
                                        && error.should_report_to_peer()
                                    {
                                        send_failure(
                                            &session.outgoing,
                                            connection_id,
                                            FailureCode::NegotiationFailed,
                                            error.peer_detail(),
                                        ).await;
                                    }
                                    let _ = done_tx.send(connection_id).await;
                                }
                                .instrument(info_span!("P2pAnswer", %connection_id)),
                            );
                        }
                        message => {
                            let Some(connection_id) = message.connection_id() else {
                                return Err(P2pServerError::Protocol("Unexpected hello message".into()));
                            };
                            if let Some(session) = sessions.get(&connection_id)
                                && session.try_send(message).is_err()
                            {
                                sessions.remove(&connection_id);
                                send_failure(
                                    &outgoing,
                                    connection_id,
                                    FailureCode::CapacityExceeded,
                                    "P2P session signaling queue is full",
                                ).await;
                            }
                        }
                    }
                }
                done = done_rx.recv() => {
                    if let Some(done) = done {
                        sessions.remove(&done);
                    }
                }
                () = self.shutdown.clone() => return Ok(()),
            }
        }
    }

    /// Serves the gateway's existing TLS and Axum stack on one reliable channel.
    async fn serve_p2p_connection(
        self: Arc<Self>,
        connection: DataChannelIo,
    ) -> Result<(), P2pServerError> {
        let tls = self.p2p_tls_server.accept(connection).await?;
        let service = TowerToHyperService::new(self.make_app());
        let mut builder = auto::Builder::new(TokioExecutor::new());
        builder
            .http1()
            .timer(TokioTimer::new())
            .header_read_timeout(HTTP_TIMEOUT);
        builder
            .http2()
            .timer(TokioTimer::new())
            .keep_alive_timeout(HTTP_TIMEOUT);
        builder
            .serve_connection(TokioIo::new(tls), service)
            .await
            .map_err(P2pServerError::ServeHttp)
    }
}

// TODO: move AnswerSession and its implementation to remote/server/src/server/p2p/server/answer.rs
struct AnswerSession {
    server: Arc<Server>,
    config: P2pRegistrationConfig,
    connection_id: P2pConnectionId,
    incoming: mpsc::Receiver<SignalMessage>,
    outgoing: mpsc::Sender<SignalMessage>,
    permit: Option<OwnedSemaphorePermit>,
}

impl AnswerSession {
    async fn run(&mut self) -> Result<(), P2pServerError> {
        let (local_ice_tx, mut local_ice_rx) = mpsc::channel(ICE_QUEUE_CAPACITY);
        let peer = PeerConnectionBuilder::new(self.config.ice_servers.clone(), local_ice_tx)
            .build()
            .await?;
        let mut data_channel = tokio::spawn({
            let peer = peer.clone();
            async move { peer.accept_reliable_data_channel().await?.wait_open().await }
        });
        let timeout = tokio::time::sleep(self.config.handshake_timeout);
        tokio::pin!(timeout);
        let mut remote_offer_set = false;

        let result = loop {
            tokio::select! {
                local_ice = local_ice_rx.recv() => {
                    let Some(local_ice) = local_ice else {
                        break Err(P2pServerError::Protocol("Local ICE stream closed".into()));
                    };
                    let message = match local_ice {
                        LocalIceEvent::Candidate(candidate) => SignalMessage::IceCandidate {
                            connection_id: self.connection_id,
                            candidate,
                        },
                        LocalIceEvent::EndOfCandidates => SignalMessage::EndOfCandidates {
                            connection_id: self.connection_id,
                        },
                        LocalIceEvent::Error(error) => break Err(P2pServerError::Protocol(error)),
                    };
                    self.outgoing.send(message).await.map_err(|_| P2pServerError::RegistrationClosed)?;
                }
                incoming = self.incoming.recv() => {
                    let Some(incoming) = incoming else {
                        break Err(P2pServerError::RegistrationClosed);
                    };
                    incoming.validate()?;
                    match incoming {
                        SignalMessage::Description {
                            connection_id,
                            description: description @ SessionDescription::Offer(_),
                        } if connection_id == self.connection_id && !remote_offer_set => {
                            peer.set_remote_description(description).await?;
                            remote_offer_set = true;
                            let description = peer.create_answer().await?;
                            self.outgoing.send(SignalMessage::Description {
                                connection_id: self.connection_id,
                                description,
                            }).await.map_err(|_| P2pServerError::RegistrationClosed)?;
                        }
                        SignalMessage::IceCandidate { connection_id, candidate }
                            if connection_id == self.connection_id =>
                        {
                            peer.add_remote_candidate(candidate).await?;
                        }
                        SignalMessage::EndOfCandidates { connection_id }
                            if connection_id == self.connection_id =>
                        {
                            peer.end_remote_candidates().await?;
                        }
                        SignalMessage::Cancel { connection_id }
                            if connection_id == self.connection_id =>
                        {
                            break Err(P2pServerError::PeerCancelled("Client cancelled".into()));
                        }
                        SignalMessage::Failure { connection_id, detail, .. }
                            if connection_id == self.connection_id =>
                        {
                            break Err(P2pServerError::PeerCancelled(detail));
                        }
                        _ => break Err(P2pServerError::Protocol("Unexpected session message".into())),
                    }
                }
                opened = &mut data_channel => {
                    let connection = opened.map_err(P2pServerError::SessionTask)??;
                    let server = self.server.clone();
                    let shutdown = server.shutdown.clone();
                    let peer = peer.clone();
                    let permit = self.permit.take().expect("session permit");
                    tokio::spawn(async move {
                        let _permit = permit;
                        tokio::select! {
                            result = server.clone().serve_p2p_connection(connection) => {
                                if let Err(error) = result {
                                    warn!(%error, "P2P HTTP connection failed");
                                }
                            }
                            () = shutdown => {}
                        }
                        let _ = peer.close().await;
                    }.in_current_span());
                    break Ok(());
                }
                () = &mut timeout => break Err(P2pServerError::HandshakeTimeout),
                () = self.server.shutdown.clone() => break Err(P2pServerError::Shutdown),
            }
        };
        data_channel.abort();
        if result.is_err() {
            let _ = peer.close().await;
        }
        result
    }
}

fn registration_request(
    config: &P2pRegistrationConfig,
) -> Result<tungstenite::http::Request<()>, P2pServerError> {
    let mut url = Url::parse(&config.signaling_url)?;
    match url.scheme() {
        "http" => url
            .set_scheme("ws")
            .map_err(|_| P2pServerError::InvalidUrlScheme)?,
        "https" => url
            .set_scheme("wss")
            .map_err(|_| P2pServerError::InvalidUrlScheme)?,
        "ws" | "wss" => {}
        _ => return Err(P2pServerError::InvalidUrlScheme),
    }
    url.path_segments_mut()
        .map_err(|_| P2pServerError::InvalidUrlScheme)?
        .pop_if_empty()
        .extend(["p2p", "register", config.server_name.as_ref()]);
    let mut request = url.as_str().into_client_request()?;
    if let Some(P2pRegistrationAuthorization::BearerToken(token)) = &config.authorization {
        let value = tungstenite::http::HeaderValue::from_str(&format!("Bearer {token}"))?;
        request
            .headers_mut()
            .insert(tungstenite::http::header::AUTHORIZATION, value);
    }
    Ok(request)
}

fn parse_signal_text(text: &str) -> Result<SignalMessage, P2pServerError> {
    let message: SignalMessage = serde_json::from_str(text)?;
    message.validate()?;
    Ok(message)
}

async fn send_signal<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    message: &SignalMessage,
) -> Result<(), P2pServerError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    socket
        .send(Message::Text(serde_json::to_string(message)?.into()))
        .await?;
    Ok(())
}

async fn send_failure(
    outgoing: &mpsc::Sender<SignalMessage>,
    connection_id: P2pConnectionId,
    code: FailureCode,
    detail: &str,
) {
    let mut detail = detail.to_owned();
    if detail.len() > MAX_FAILURE_DETAIL_LEN {
        let mut end = MAX_FAILURE_DETAIL_LEN;
        while !detail.is_char_boundary(end) {
            end -= 1;
        }
        detail.truncate(end);
    }
    let _ = outgoing
        .send(SignalMessage::Failure {
            connection_id,
            code,
            detail,
        })
        .await;
}

// TODO: blank space between enum values please, like I did for the first values.
// TODO: move P2pServerError to remote/server/src/server/p2p/server/error.rs
#[derive(Debug, thiserror::Error)]
enum P2pServerError {
    #[error("Invalid P2P server configuration: {0}")]
    InvalidConfig(String),

    #[error("Invalid signaling URL scheme")]
    InvalidUrlScheme,

    #[error("Signaling URL: {0}")]
    Url(#[from] url::ParseError),

    #[error("Signaling request: {0}")]
    Request(#[from] tungstenite::http::Error),
    #[error("Signaling authorization header: {0}")]
    Header(#[from] tungstenite::http::header::InvalidHeaderValue),
    #[error("Signaling WebSocket: {0}")]
    WebSocket(#[from] tungstenite::Error),
    #[error("Signaling JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Signaling protocol: {0}")]
    Validation(#[from] trz_gateway_common::p2p::protocol::ValidationError),
    #[error("Signaling protocol: {0}")]
    Protocol(String),
    #[error("Signaling registration closed")]
    RegistrationClosed,
    #[error("Peer cancelled: {0}")]
    PeerCancelled(String),
    #[error("WebRTC handshake timed out")]
    HandshakeTimeout,
    #[error("Server is shutting down")]
    Shutdown,
    #[error("WebRTC: {0}")]
    PeerConnection(#[from] trz_gateway_common::p2p::peer_connection::PeerConnectionError),
    #[error("WebRTC session task: {0}")]
    SessionTask(tokio::task::JoinError),
    #[error("P2P TLS: {0}")]
    Tls(#[from] std::io::Error),
    #[error("P2P HTTP: {0}")]
    ServeHttp(Box<dyn std::error::Error + Send + Sync>),
}

impl P2pServerError {
    fn should_report_to_peer(&self) -> bool {
        !matches!(
            self,
            Self::RegistrationClosed | Self::PeerCancelled(_) | Self::Shutdown
        )
    }

    fn peer_detail(&self) -> &'static str {
        match self {
            Self::HandshakeTimeout => {
                "WebRTC handshake timed out; configure TURN when direct ICE is unavailable"
            }
            Self::Protocol(_) | Self::Validation(_) => "Invalid P2P signaling message",
            Self::PeerConnection(_) | Self::SessionTask(_) => {
                "WebRTC negotiation failed; check STUN/TURN configuration"
            }
            _ => "P2P session failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_encoded_registration_url_and_redacted_authorization() {
        let mut config = P2pRegistrationConfig::new(
            "https://signal.example/base/",
            "server name/with slash".into(),
        );
        config.authorization = Some(P2pRegistrationAuthorization::BearerToken("secret".into()));
        let request = registration_request(&config).unwrap();
        assert_eq!(
            "wss://signal.example/base/p2p/register/server%20name%2Fwith%20slash",
            request.uri().to_string(),
        );
        assert_eq!("Bearer secret", request.headers()["authorization"]);
    }
}
