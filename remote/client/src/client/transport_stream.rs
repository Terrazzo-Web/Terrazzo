use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use pin_project::pin_project;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::ReadBuf;
use tokio::net::TcpStream;

use crate::p2p::P2pStream;

#[pin_project(project = TransportStreamProj)]
pub enum TransportStream {
    Direct(#[pin] TcpStream),
    WebRtc(#[pin] P2pStream),
}

impl AsyncRead for TransportStream {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.project() {
            TransportStreamProj::Direct(stream) => stream.poll_read(context, buffer),
            TransportStreamProj::WebRtc(stream) => stream.poll_read(context, buffer),
        }
    }
}

impl AsyncWrite for TransportStream {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.project() {
            TransportStreamProj::Direct(stream) => stream.poll_write(context, buffer),
            TransportStreamProj::WebRtc(stream) => stream.poll_write(context, buffer),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.project() {
            TransportStreamProj::Direct(stream) => stream.poll_flush(context),
            TransportStreamProj::WebRtc(stream) => stream.poll_flush(context),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.project() {
            TransportStreamProj::Direct(stream) => stream.poll_shutdown(context),
            TransportStreamProj::WebRtc(stream) => stream.poll_shutdown(context),
        }
    }
}
