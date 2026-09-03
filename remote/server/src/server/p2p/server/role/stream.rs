use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::ReadBuf;
use tokio::sync::OwnedSemaphorePermit;
use trz_gateway_common::p2p::data_channel_io::DataChannelIo;
use trz_gateway_common::p2p::peer_connection::PeerConnection;

/// Keeps the peer and session permit alive after Hyper transfers the IO to an
/// upgraded WebSocket task. The HTTP future completes when the upgrade occurs,
/// before the tunnel using the byte stream has finished.
pub struct P2pServerStream {
    io: DataChannelIo,
    peer: Option<PeerConnection>,
    _permit: OwnedSemaphorePermit,
}

impl P2pServerStream {
    pub fn new(io: DataChannelIo, peer: PeerConnection, permit: OwnedSemaphorePermit) -> Self {
        Self {
            io,
            peer: Some(peer),
            _permit: permit,
        }
    }
}

impl Drop for P2pServerStream {
    fn drop(&mut self) {
        let Some(peer) = self.peer.take() else {
            return;
        };
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = peer.close().await;
            });
        }
    }
}

impl AsyncRead for P2pServerStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.io).poll_read(context, buffer)
    }
}

impl AsyncWrite for P2pServerStream {
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
