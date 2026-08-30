use std::collections::HashMap;
use std::sync::Arc;

use futures::SinkExt as _;
use futures::StreamExt as _;
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tracing::Instrument as _;
use tracing::info_span;
use trz_gateway_common::p2p::protocol::FailureCode;
use trz_gateway_common::p2p::protocol::MAX_FAILURE_DETAIL_LEN;
use trz_gateway_common::p2p::protocol::P2pConnectionId;
use trz_gateway_common::p2p::protocol::SignalMessage;

use crate::server::Server;
use crate::server::gateway_config::p2p::P2pRegistrationConfig;
use crate::server::p2p::server::answer::AnswerSession;
use crate::server::p2p::server::error::P2pServerError;
use crate::server::p2p::server::role::REGISTRATION_QUEUE_CAPACITY;

use super::SESSION_QUEUE_CAPACITY;

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

fn parse_signal_text(text: &str) -> Result<SignalMessage, P2pServerError> {
    let message: SignalMessage = serde_json::from_str(text)?;
    message.validate()?;
    Ok(message)
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
