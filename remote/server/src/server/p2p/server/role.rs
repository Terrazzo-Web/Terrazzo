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
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tracing::Instrument as _;
use tracing::info;
use tracing::info_span;
use tracing::warn;
use trz_gateway_common::p2p::data_channel_io::DataChannelIo;
use trz_gateway_common::p2p::protocol::FailureCode;
use trz_gateway_common::p2p::protocol::MAX_FAILURE_DETAIL_LEN;
use trz_gateway_common::p2p::protocol::MAX_SDP_LEN;
use trz_gateway_common::p2p::protocol::P2pConnectionId;
use trz_gateway_common::p2p::protocol::PROTOCOL_VERSION;
use trz_gateway_common::p2p::protocol::SignalMessage;
use url::Url;

use super::answer::AnswerSession;
use super::error::P2pServerError;
use crate::server::HTTP_TIMEOUT;
use crate::server::Server;
use crate::server::gateway_config::p2p::P2pRegistrationAuthorization;
use crate::server::gateway_config::p2p::P2pRegistrationConfig;

const REGISTRATION_QUEUE_CAPACITY: usize = 128;
const SESSION_QUEUE_CAPACITY: usize = 64;
const MAX_SIGNAL_MESSAGE_SIZE: usize = MAX_SDP_LEN + 64 * 1024;

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

        Registration::new(self, config).run(socket).await
    }

    /// Serves the gateway's existing TLS and Axum stack on one reliable channel.
    pub(super) async fn serve_p2p_connection(
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

struct Registration {
    server: Arc<Server>,
    config: P2pRegistrationConfig,
    semaphore: Arc<Semaphore>,
    outgoing: mpsc::Sender<SignalMessage>,
    outgoing_rx: mpsc::Receiver<SignalMessage>,
    done_tx: mpsc::Sender<P2pConnectionId>,
    done_rx: mpsc::Receiver<P2pConnectionId>,
    sessions: HashMap<P2pConnectionId, mpsc::Sender<SignalMessage>>,
}

impl Registration {
    fn new(server: Arc<Server>, config: P2pRegistrationConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_sessions));
        let (outgoing, outgoing_rx) = mpsc::channel(REGISTRATION_QUEUE_CAPACITY);
        let (done_tx, done_rx) = mpsc::channel(config.max_sessions);
        Self {
            server,
            config,
            semaphore,
            outgoing,
            outgoing_rx,
            done_tx,
            done_rx,
            sessions: HashMap::new(),
        }
    }

    async fn run(
        mut self,
        mut socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    ) -> Result<(), P2pServerError> {
        loop {
            tokio::select! {
                outgoing = self.outgoing_rx.recv() => {
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
                    self.handle_message(message).await?;
                }
                done = self.done_rx.recv() => {
                    if let Some(done) = done {
                        self.sessions.remove(&done);
                    }
                }
                () = self.server.shutdown.clone() => return Ok(()),
            }
        }
    }

    async fn handle_message(&mut self, message: SignalMessage) -> Result<(), P2pServerError> {
        match message {
            SignalMessage::Start { connection_id } => {
                self.start_session(connection_id).await;
                Ok(())
            }
            message => self.forward_to_session(message).await,
        }
    }

    async fn start_session(&mut self, connection_id: P2pConnectionId) {
        if self.sessions.contains_key(&connection_id) {
            send_failure(
                &self.outgoing,
                connection_id,
                FailureCode::InvalidMessage,
                "Duplicate session start",
            )
            .await;
            return;
        }
        let Ok(permit) = self.semaphore.clone().try_acquire_owned() else {
            send_failure(
                &self.outgoing,
                connection_id,
                FailureCode::CapacityExceeded,
                "This server has too many active P2P sessions",
            )
            .await;
            return;
        };
        let (session_tx, session_rx) = mpsc::channel(SESSION_QUEUE_CAPACITY);
        self.sessions.insert(connection_id, session_tx);
        let mut session = AnswerSession::new(
            self.server.clone(),
            self.config.clone(),
            connection_id,
            session_rx,
            self.outgoing.clone(),
            permit,
        );
        let outgoing = self.outgoing.clone();
        let done_tx = self.done_tx.clone();
        tokio::spawn(
            async move {
                if let Err(error) = session.run().await
                    && error.should_report_to_peer()
                {
                    send_failure(
                        &outgoing,
                        connection_id,
                        FailureCode::NegotiationFailed,
                        error.peer_detail(),
                    )
                    .await;
                }
                let _ = done_tx.send(connection_id).await;
            }
            .instrument(info_span!("P2pAnswer", %connection_id)),
        );
    }

    async fn forward_to_session(&mut self, message: SignalMessage) -> Result<(), P2pServerError> {
        let Some(connection_id) = message.connection_id() else {
            return Err(P2pServerError::Protocol("Unexpected hello message".into()));
        };
        if let Some(session) = self.sessions.get(&connection_id)
            && session.try_send(message).is_err()
        {
            self.sessions.remove(&connection_id);
            send_failure(
                &self.outgoing,
                connection_id,
                FailureCode::CapacityExceeded,
                "P2P session signaling queue is full",
            )
            .await;
        }
        Ok(())
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
    socket: &mut WebSocketStream<S>,
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
