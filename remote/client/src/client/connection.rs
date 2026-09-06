use std::io::ErrorKind;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

use futures::task::AtomicWaker;
use pin_project::pin_project;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::ReadBuf;
use tokio_rustls::server::TlsStream;
use tonic::transport::server::Connected;

struct ForceCloseState {
    closed: AtomicBool,
    waker: AtomicWaker,
}

/// I/O wrapper that allows a connection owner to interrupt pending transport
/// operations instead of waiting indefinitely for graceful protocol shutdown.
#[pin_project]
pub(super) struct ForceCloseIo<T> {
    #[pin]
    inner: T,
    state: Arc<ForceCloseState>,
}

#[derive(Clone)]
pub(super) struct ForceCloseHandle(Arc<ForceCloseState>);

impl<T> ForceCloseIo<T> {
    pub(super) fn new(inner: T) -> (Self, ForceCloseHandle) {
        let state = Arc::new(ForceCloseState {
            closed: AtomicBool::new(false),
            waker: AtomicWaker::new(),
        });
        (
            Self {
                inner,
                state: state.clone(),
            },
            ForceCloseHandle(state),
        )
    }
}

impl Drop for ForceCloseHandle {
    fn drop(&mut self) {
        self.0.closed.store(true, Ordering::Release);
        self.0.waker.wake();
    }
}

fn is_closed(state: &ForceCloseState, context: &Context<'_>) -> bool {
    if state.closed.load(Ordering::Acquire) {
        return true;
    }
    state.waker.register(context.waker());
    state.closed.load(Ordering::Acquire)
}

fn closed_error() -> std::io::Error {
    std::io::Error::new(
        ErrorKind::ConnectionAborted,
        "connection was forcefully closed",
    )
}

impl<T: AsyncRead> AsyncRead for ForceCloseIo<T> {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.project();
        if is_closed(this.state, context) {
            return Poll::Ready(Ok(()));
        }
        this.inner.poll_read(context, buffer)
    }
}

impl<T: AsyncWrite> AsyncWrite for ForceCloseIo<T> {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.project();
        if is_closed(this.state, context) {
            return Poll::Ready(Err(closed_error()));
        }
        this.inner.poll_write(context, buffer)
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.project();
        if is_closed(this.state, context) {
            return Poll::Ready(Err(closed_error()));
        }
        this.inner.poll_flush(context)
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.project();
        if is_closed(this.state, context) {
            return Poll::Ready(Ok(()));
        }
        this.inner.poll_shutdown(context)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffers: &[std::io::IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.project();
        if is_closed(this.state, context) {
            return Poll::Ready(Err(closed_error()));
        }
        this.inner.poll_write_vectored(context, buffers)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
}

/// A wrapper for [TlsStream] that implements [Connected].
#[pin_project]
pub struct Connection<C> {
    #[pin]
    tls_stream: TlsStream<C>,
}

impl<C> Connection<C> {
    pub fn new(tls_stream: TlsStream<C>) -> Self {
        Self { tls_stream }
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::io::AsyncReadExt as _;
    use tokio::io::AsyncWriteExt as _;

    use super::*;

    #[tokio::test]
    async fn force_close_interrupts_pending_read_and_rejects_writes() {
        let (io, _remote) = tokio::io::duplex(64);
        let (mut io, close) = ForceCloseIo::new(io);
        let read = tokio::spawn(async move {
            let mut byte = [0];
            io.read(&mut byte).await
        });
        tokio::task::yield_now().await;

        drop(close);

        assert_eq!(
            0,
            tokio::time::timeout(Duration::from_secs(1), read)
                .await
                .unwrap()
                .unwrap()
                .unwrap()
        );

        let (io, _remote) = tokio::io::duplex(64);
        let (mut io, close) = ForceCloseIo::new(io);
        drop(close);
        assert_eq!(
            ErrorKind::ConnectionAborted,
            io.write_all(b"data").await.unwrap_err().kind()
        );
    }
}
