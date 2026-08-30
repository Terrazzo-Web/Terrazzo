//! Client-side WebRTC signaling and reliable byte-stream transport.

use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use futures::SinkExt as _;
use futures::StreamExt as _;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::ReadBuf;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use trz_gateway_common::p2p::data_channel_io::DataChannelIo;
use trz_gateway_common::p2p::peer_connection::LocalIceEvent;
use trz_gateway_common::p2p::peer_connection::PeerConnection;
use trz_gateway_common::p2p::peer_connection::PeerConnectionBuilder;
use trz_gateway_common::p2p::protocol::FailureCode;
use trz_gateway_common::p2p::protocol::MAX_SDP_LEN;
use trz_gateway_common::p2p::protocol::P2pConnectionId;
use trz_gateway_common::p2p::protocol::PROTOCOL_VERSION;
use trz_gateway_common::p2p::protocol::SessionDescription;
use trz_gateway_common::p2p::protocol::SignalMessage;
use url::Url;

use crate::client::config::P2pClientConfig;

const ICE_QUEUE_CAPACITY: usize = 64;
const MAX_SIGNAL_MESSAGE_SIZE: usize = MAX_SDP_LEN + 64 * 1024;

/// Opens one fresh reliable P2P byte stream.
pub(crate) async fn connect(config: &P2pClientConfig) -> Result<P2pStream, P2pConnectError> {
    tokio::time::timeout(config.connect_timeout, connect_inner(config))
        .await
        .map_err(|_| P2pConnectError::Timeout("complete P2P connection"))?
}

async fn connect_inner(config: &P2pClientConfig) -> Result<P2pStream, P2pConnectError> {
    let request = signaling_request(config)?;
    let websocket_config = tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(MAX_SIGNAL_MESSAGE_SIZE))
        .max_frame_size(Some(MAX_SIGNAL_MESSAGE_SIZE));
    let (mut socket, _) = tokio::time::timeout(
        config.signaling_timeout,
        connect_async_with_config(request, Some(websocket_config), false),
    )
    .await
    .map_err(|_| P2pConnectError::Timeout("signaling WebSocket"))??;
    send_signal(
        &mut socket,
        &SignalMessage::Hello {
            protocol_version: PROTOCOL_VERSION,
        },
    )
    .await?;
    let connection_id = receive_start(&mut socket, config).await?;

    let (local_ice_tx, mut local_ice_rx) = mpsc::channel(ICE_QUEUE_CAPACITY);
    let peer = PeerConnectionBuilder::new(config.ice_servers.clone(), local_ice_tx)
        .build()
        .await?;
    let peer = PeerGuard::new(peer);
    let data_channel = peer.peer().create_reliable_data_channel().await?;
    let offer = peer.peer().create_offer().await?;
    send_signal(
        &mut socket,
        &SignalMessage::Description {
            connection_id,
            description: offer,
        },
    )
    .await?;

    let result = negotiate(
        &mut socket,
        config,
        connection_id,
        peer.peer(),
        &mut local_ice_rx,
        data_channel.wait_open(),
    )
    .await;
    let stream = match result {
        Ok(stream) => peer.into_stream(stream),
        Err(error) => {
            let _ = send_signal(&mut socket, &SignalMessage::Cancel { connection_id }).await;
            let _ = socket.close(None).await;
            return Err(error);
        }
    };
    let _ = socket.close(None).await;
    Ok(stream)
}

async fn receive_start<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    config: &P2pClientConfig,
) -> Result<P2pConnectionId, P2pConnectError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    tokio::time::timeout(config.signaling_timeout, async {
        loop {
            match socket
                .next()
                .await
                .ok_or(P2pConnectError::SignalingClosed)??
            {
                Message::Text(text) => match parse_signal(&text)? {
                    SignalMessage::Start { connection_id } => return Ok(connection_id),
                    _ => return Err(P2pConnectError::Protocol("Expected session start".into())),
                },
                Message::Ping(_) | Message::Pong(_) => socket.flush().await?,
                Message::Close(_) => return Err(P2pConnectError::SignalingClosed),
                Message::Binary(_) | Message::Frame(_) => {
                    return Err(P2pConnectError::Protocol(
                        "Signaling messages must be JSON text".into(),
                    ));
                }
            }
        }
    })
    .await
    .map_err(|_| P2pConnectError::Timeout("signaling session start"))?
}

async fn negotiate<S, F>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    config: &P2pClientConfig,
    connection_id: P2pConnectionId,
    peer: &PeerConnection,
    local_ice_rx: &mut mpsc::Receiver<LocalIceEvent>,
    opened: F,
) -> Result<DataChannelIo, P2pConnectError>
where
    S: AsyncRead + AsyncWrite + Unpin,
    F: Future<
        Output = Result<
            DataChannelIo,
            trz_gateway_common::p2p::peer_connection::PeerConnectionError,
        >,
    >,
{
    tokio::pin!(opened);
    let timeout = tokio::time::sleep(config.handshake_timeout);
    tokio::pin!(timeout);
    let mut answer_received = false;
    loop {
        tokio::select! {
            local_ice = local_ice_rx.recv() => {
                let message = match local_ice.ok_or(P2pConnectError::LocalIceClosed)? {
                    LocalIceEvent::Candidate(candidate) => SignalMessage::IceCandidate {
                        connection_id,
                        candidate,
                    },
                    LocalIceEvent::EndOfCandidates => SignalMessage::EndOfCandidates {
                        connection_id,
                    },
                    LocalIceEvent::Error(error) => return Err(P2pConnectError::Protocol(error)),
                };
                send_signal(socket, &message).await?;
            }
            incoming = socket.next() => {
                let message = match incoming.ok_or(P2pConnectError::SignalingClosed)?? {
                    Message::Text(text) => parse_signal(&text)?,
                    Message::Ping(_) | Message::Pong(_) => {
                        socket.flush().await?;
                        continue;
                    }
                    Message::Close(_) => return Err(P2pConnectError::SignalingClosed),
                    Message::Binary(_) | Message::Frame(_) => {
                        return Err(P2pConnectError::Protocol(
                            "Signaling messages must be JSON text".into(),
                        ));
                    }
                };
                if message.connection_id() != Some(connection_id) {
                    return Err(P2pConnectError::Protocol("Unexpected connection identifier".into()));
                }
                match message {
                    SignalMessage::Description {
                        description: description @ SessionDescription::Answer(_),
                        ..
                    } if !answer_received => {
                        peer.set_remote_description(description).await?;
                        answer_received = true;
                    }
                    SignalMessage::IceCandidate { candidate, .. } => {
                        peer.add_remote_candidate(candidate).await?;
                    }
                    SignalMessage::EndOfCandidates { .. } => {
                        peer.end_remote_candidates().await?;
                    }
                    SignalMessage::Cancel { .. } => return Err(P2pConnectError::PeerCancelled),
                    SignalMessage::Failure { code, detail, .. } => {
                        return Err(P2pConnectError::PeerFailure { code, detail });
                    }
                    _ => return Err(P2pConnectError::Protocol("Unexpected session message".into())),
                }
            }
            opened = &mut opened => return Ok(opened?),
            () = &mut timeout => return Err(P2pConnectError::Timeout("WebRTC handshake")),
        }
    }
}

fn signaling_request(
    config: &P2pClientConfig,
) -> Result<tungstenite::http::Request<()>, P2pConnectError> {
    let mut url = Url::parse(&config.signaling_url)?;
    match url.scheme() {
        "http" => url
            .set_scheme("ws")
            .map_err(|_| P2pConnectError::InvalidUrlScheme)?,
        "https" => url
            .set_scheme("wss")
            .map_err(|_| P2pConnectError::InvalidUrlScheme)?,
        "ws" | "wss" => {}
        _ => return Err(P2pConnectError::InvalidUrlScheme),
    }
    url.path_segments_mut()
        .map_err(|_| P2pConnectError::InvalidUrlScheme)?
        .pop_if_empty()
        .extend(["p2p", "connect", config.server_name.as_ref()]);
    Ok(url.as_str().into_client_request()?)
}

fn parse_signal(text: &str) -> Result<SignalMessage, P2pConnectError> {
    let message: SignalMessage = serde_json::from_str(text)?;
    message.validate()?;
    Ok(message)
}

async fn send_signal<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    message: &SignalMessage,
) -> Result<(), P2pConnectError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    socket
        .send(Message::Text(serde_json::to_string(message)?.into()))
        .await?;
    Ok(())
}

struct PeerGuard(Option<PeerConnection>);

impl PeerGuard {
    fn new(peer: PeerConnection) -> Self {
        Self(Some(peer))
    }

    fn peer(&self) -> &PeerConnection {
        self.0.as_ref().expect("peer guard")
    }

    fn into_stream(mut self, io: DataChannelIo) -> P2pStream {
        P2pStream {
            io,
            peer: self.0.take(),
        }
    }
}

impl Drop for PeerGuard {
    fn drop(&mut self) {
        close_peer(self.0.take());
    }
}

/// Reliable WebRTC stream that keeps its peer connection alive.
pub(crate) struct P2pStream {
    io: DataChannelIo,
    peer: Option<PeerConnection>,
}

impl Drop for P2pStream {
    fn drop(&mut self) {
        close_peer(self.peer.take());
    }
}

fn close_peer(peer: Option<PeerConnection>) {
    if let Some(peer) = peer
        && let Ok(runtime) = tokio::runtime::Handle::try_current()
    {
        runtime.spawn(async move {
            let _ = peer.close().await;
        });
    }
}

impl AsyncRead for P2pStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.io).poll_read(context, buffer)
    }
}

impl AsyncWrite for P2pStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(context, buffer)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(context)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.io).poll_shutdown(context)
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum P2pConnectError {
    #[error("[{n}] Invalid signaling URL scheme", n = self.name())]
    InvalidUrlScheme,

    #[error("[{n}] Invalid signaling URL: {0}", n = self.name())]
    Url(#[from] url::ParseError),

    #[error("[{n}] Failed to build signaling request: {0}", n = self.name())]
    Request(#[from] tungstenite::http::Error),

    #[error("[{n}] Signaling WebSocket failed: {0}", n = self.name())]
    WebSocket(#[from] tungstenite::Error),

    #[error("[{n}] Invalid signaling JSON: {0}", n = self.name())]
    Json(#[from] serde_json::Error),

    #[error("[{n}] Invalid signaling protocol message: {0}", n = self.name())]
    Validation(#[from] trz_gateway_common::p2p::protocol::ValidationError),

    #[error("[{n}] Signaling protocol error: {0}", n = self.name())]
    Protocol(String),

    #[error("[{n}] Signaling connection closed", n = self.name())]
    SignalingClosed,

    #[error("[{n}] Local ICE event stream closed", n = self.name())]
    LocalIceClosed,

    #[error("[{n}] Peer cancelled the P2P connection", n = self.name())]
    PeerCancelled,

    #[error("[{n}] Peer rejected the P2P connection ({code:?}): {detail}", n = self.name())]
    PeerFailure { code: FailureCode, detail: String },

    #[error("[{n}] {0}", n = self.name())]
    PeerConnection(#[from] trz_gateway_common::p2p::peer_connection::PeerConnectionError),

    #[error("[{n}] Timed out while establishing {0}", n = self.name())]
    Timeout(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_encoded_signaling_url() {
        let config = P2pClientConfig::new(
            "https://signal.example/base/",
            "server name/with slash".into(),
        );
        let request = signaling_request(&config).unwrap();
        assert_eq!(
            "wss://signal.example/base/p2p/connect/server%20name%2Fwith%20slash",
            request.uri().to_string(),
        );
    }
}
