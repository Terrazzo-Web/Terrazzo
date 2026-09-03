use std::sync::Arc;

use autoclone::autoclone;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::mpsc;
use tokio::task::JoinError;
use tracing::Instrument as _;
use tracing::warn;
use trz_gateway_common::p2p::data_channel_io::DataChannelIo;
use trz_gateway_common::p2p::peer_connection::LocalIceEvent;
use trz_gateway_common::p2p::peer_connection::PeerConnection;
use trz_gateway_common::p2p::peer_connection::PeerConnectionBuilder;
use trz_gateway_common::p2p::peer_connection::PeerConnectionError;
use trz_gateway_common::p2p::protocol::P2pConnectionId;
use trz_gateway_common::p2p::protocol::SessionDescription;
use trz_gateway_common::p2p::protocol::SignalMessage;

use super::error::P2pServerError;
use super::role::P2pServerStream;
use crate::server::Server;
use crate::server::gateway_config::p2p::P2pRegistrationConfig;

const ICE_QUEUE_CAPACITY: usize = 64;

pub struct AnswerSession {
    server: Arc<Server>,
    config: P2pRegistrationConfig,
    connection_id: P2pConnectionId,
    incoming: mpsc::Receiver<SignalMessage>,
    outgoing: mpsc::Sender<SignalMessage>,
    permit: Option<OwnedSemaphorePermit>,
}

#[autoclone]
impl AnswerSession {
    pub fn new(
        server: Arc<Server>,
        config: P2pRegistrationConfig,
        connection_id: P2pConnectionId,
        incoming: mpsc::Receiver<SignalMessage>,
        outgoing: mpsc::Sender<SignalMessage>,
        permit: OwnedSemaphorePermit,
    ) -> Self {
        Self {
            server,
            config,
            connection_id,
            incoming,
            outgoing,
            permit: Some(permit),
        }
    }

    pub async fn run(&mut self) -> Result<(), P2pServerError> {
        let (local_ice_tx, mut local_ice_rx) = mpsc::channel(ICE_QUEUE_CAPACITY);
        let peer = PeerConnectionBuilder::new(self.config.ice_servers.clone(), local_ice_tx)
            .build()
            .await?;
        let data_channel = tokio::spawn(async move {
            autoclone!(peer);
            peer.accept_reliable_data_channel().await?.wait_open().await
        });
        let mut data_channel = scopeguard::guard(data_channel, |data_channel| {
            data_channel.abort();
        });

        let timeout = tokio::time::sleep(self.config.handshake_timeout);
        tokio::pin!(timeout);
        let mut remote_offer_set = false;

        let result = async {
            loop {
                tokio::select! {
                    local_ice = local_ice_rx.recv() => self.handle_local_ice(local_ice).await?,
                    incoming = self.incoming.recv() => self.handle_incoming(incoming, &peer, &mut remote_offer_set).await?,
                    opened = &mut *data_channel => {
                       let () = self.handle_opened(opened, &peer)?;
                       break Ok(());
                    }
                    () = &mut timeout => break Err(P2pServerError::HandshakeTimeout),
                    () = self.server.shutdown.clone() => break Err(P2pServerError::Shutdown),
                }
            }
        }.await;
        if result.is_err() {
            let _closed = peer.close().await;
        }
        result
    }

    async fn handle_local_ice(
        &self,
        local_ice: Option<LocalIceEvent>,
    ) -> Result<(), P2pServerError> {
        let Some(local_ice) = local_ice else {
            return Err(P2pServerError::Protocol("Local ICE stream closed".into()));
        };
        let message = match local_ice {
            LocalIceEvent::Candidate(candidate) => SignalMessage::IceCandidate {
                connection_id: self.connection_id,
                candidate,
            },
            LocalIceEvent::EndOfCandidates => SignalMessage::EndOfCandidates {
                connection_id: self.connection_id,
            },
            LocalIceEvent::Error(error) => return Err(P2pServerError::Protocol(error)),
        };
        self.outgoing
            .send(message)
            .await
            .map_err(|_send_error| P2pServerError::RegistrationClosed)?;
        Ok(())
    }

    async fn handle_incoming(
        &self,
        incoming: Option<SignalMessage>,
        peer: &PeerConnection,
        remote_offer_set: &mut bool,
    ) -> Result<(), P2pServerError> {
        let Some(incoming) = incoming else {
            return Err(P2pServerError::RegistrationClosed);
        };
        incoming.validate()?;
        match incoming {
            SignalMessage::Description {
                connection_id,
                description: description @ SessionDescription::Offer(_),
            } if connection_id == self.connection_id && !*remote_offer_set => {
                peer.set_remote_description(description).await?;
                *remote_offer_set = true;
                let description = peer.create_answer().await?;
                self.outgoing
                    .send(SignalMessage::Description {
                        connection_id: self.connection_id,
                        description,
                    })
                    .await
                    .map_err(|_| P2pServerError::RegistrationClosed)?;
            }
            SignalMessage::IceCandidate {
                connection_id,
                candidate,
            } if connection_id == self.connection_id => {
                peer.add_remote_candidate(candidate).await?;
            }
            SignalMessage::EndOfCandidates { connection_id }
                if connection_id == self.connection_id =>
            {
                peer.end_remote_candidates().await?;
            }
            SignalMessage::Cancel { connection_id } if connection_id == self.connection_id => {
                return Err(P2pServerError::PeerCancelled("Client cancelled".into()));
            }
            SignalMessage::Failure {
                connection_id,
                detail,
                ..
            } if connection_id == self.connection_id => {
                return Err(P2pServerError::PeerCancelled(detail));
            }
            _ => {
                return Err(P2pServerError::Protocol(
                    "Unexpected session message".into(),
                ));
            }
        }
        Ok(())
    }

    fn handle_opened(
        &mut self,
        opened: Result<Result<DataChannelIo, PeerConnectionError>, JoinError>,
        peer: &PeerConnection,
    ) -> Result<(), P2pServerError> {
        let connection = opened.map_err(P2pServerError::SessionTask)??;
        let server = self.server.clone();
        let shutdown = server.shutdown.clone();
        let permit = self.permit.take().expect("session permit");
        let connection = P2pServerStream::new(connection, peer.clone(), permit);
        tokio::spawn(
            async move {
                tokio::select! {
                    result = server.clone().serve_p2p_connection(connection) => {
                        if let Err(error) = result {
                            warn!(%error, "P2P HTTP connection failed");
                        }
                    }
                    () = shutdown => {}
                }
            }
            .in_current_span(),
        );
        Ok(())
    }
}
