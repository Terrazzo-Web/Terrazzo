use std::pin::Pin;
use std::task::Poll;

use pin_project::pin_project;
use pin_project::pinned_drop;
use tokio::sync::oneshot;

use tokio_rustls::server::TlsStream;
use tonic::transport::server::Connected;

// A wrapper for [TlsStream] that implements [Connected].
#[pin_project(PinnedDrop)]
pub struct Connection<C> {
    #[pin]
    tls_stream: TlsStream<C>,
    terminated: Option<oneshot::Sender<()>>,
}

impl<C> Connection<C> {
    pub fn new(tls_stream: TlsStream<C>, terminated: oneshot::Sender<()>) -> Self {
        Self {
            tls_stream,
            terminated: Some(terminated),
        }
    }
}

impl<C> Connected for Connection<C> {
    type ConnectInfo = ();

    fn connect_info(&self) -> Self::ConnectInfo {}
}

impl<C: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin> tokio::io::AsyncRead
    for Connection<C>
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.project().tls_stream.poll_read(cx, buf)
    }
}

impl<C: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin> tokio::io::AsyncWrite
    for Connection<C>
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.project().tls_stream.poll_write(cx, buf)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.project().tls_stream.poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.project().tls_stream.poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        self.project().tls_stream.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.tls_stream.is_write_vectored()
    }
}

#[pinned_drop]
impl<C> PinnedDrop for Connection<C> {
    fn drop(mut self: Pin<&mut Self>) {
        let _ = self
            .project()
            .terminated
            .take()
            .map(|terminated| terminated.send(()));
    }
}
