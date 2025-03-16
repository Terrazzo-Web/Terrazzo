use std::io::Cursor;
use std::pin::Pin;
use std::task::Poll;
use std::task::ready;

use axum_server::accept::Accept;
use futures::FutureExt;
use pin_project::pin_project;
use tokio::io::AsyncRead as _;
use tokio::io::ReadBuf;
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::tcp::OwnedWriteHalf;
use tracing::debug;

#[derive(Clone)]
pub struct HttpOrHttps<TLS, PLAIN> {
    pub tls: TLS,
    pub plaintext: PLAIN,
}

impl<T, P, S> Accept<TcpStream, S> for HttpOrHttps<T, P>
where
    T: Accept<PeekStream, S, Service = S> + Clone + Unpin,
    P: Accept<PeekStream, S, Service = S> + Clone + Unpin,
    S: Unpin,
{
    type Service = S;
    type Stream = TlsOrPlaintextStream<
        <T as Accept<PeekStream, S>>::Stream,
        <P as Accept<PeekStream, S>>::Stream,
    >;
    type Future = FuturePeekStream<T, P, S>;

    fn accept(&self, stream: TcpStream, service: S) -> Self::Future {
        FuturePeekStream(FuturePeekStreamImpl::Header {
            stream,
            service,
            buffer: Default::default(),
            pos: 0,
            accept: self.clone(),
        })
    }
}

pub struct FuturePeekStream<T, P, S>(FuturePeekStreamImpl<T, P, S>)
where
    T: Accept<PeekStream, S>,
    P: Accept<PeekStream, S>;

#[derive(Default)]
enum FuturePeekStreamImpl<T, P, S>
where
    T: Accept<PeekStream, S>,
    P: Accept<PeekStream, S>,
{
    Header {
        stream: TcpStream,
        service: S,
        buffer: StreamHeader,
        pos: usize,
        accept: HttpOrHttps<T, P>,
    },

    AcceptTls {
        future: Pin<Box<T::Future>>,
    },

    AcceptPlaintext {
        future: Pin<Box<P::Future>>,
    },

    #[default]
    Undefined,
}

type StreamHeader = [u8; 11];

impl<T, P, S> Future for FuturePeekStream<T, P, S>
where
    T: Accept<PeekStream, S, Service = S> + Unpin,
    P: Accept<PeekStream, S, Service = S> + Unpin,
    S: Unpin,
{
    type Output = std::io::Result<(
        TlsOrPlaintextStream<
            <T as Accept<PeekStream, S>>::Stream,
            <P as Accept<PeekStream, S>>::Stream,
        >,
        S,
    )>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        loop {
            match &mut self.0 {
                FuturePeekStreamImpl::Header {
                    stream,
                    buffer,
                    pos,
                    ..
                } => {
                    debug!(pos, ?buffer, "Polling stream header");
                    let mut buf = ReadBuf::new(buffer);
                    buf.advance(*pos);
                    ready!(Pin::new(stream).poll_read(cx, &mut buf))?;
                    *pos = buf.filled().len();
                    if *pos < buffer.len() {
                        debug!(pos, ?buffer, "Polling stream header: Continue");
                        continue;
                    }
                    debug!(pos, ?buffer, "Polling stream header: Buffer full");

                    let is_tls = &buffer[..3] == &[0x16, 0x3, 0x1] // TLS 1.0 record header
                        && buffer[5] == 1 // Client hello
                        && &buffer[9..11] == &[3, 3]; // TLS 1.2

                    let FuturePeekStreamImpl::Header {
                        stream,
                        service,
                        accept,
                        buffer,
                        ..
                    } = std::mem::take(&mut self.0)
                    else {
                        unreachable!()
                    };
                    stream.set_nodelay(true)?;
                    let (read, write) = stream.into_split();
                    let stream = PeekStream {
                        read,
                        write,
                        header: Cursor::new(buffer),
                    };
                    self.0 = if is_tls {
                        debug!("Polling TLS stream");
                        FuturePeekStreamImpl::AcceptTls {
                            future: Box::pin(accept.tls.accept(stream, service)),
                        }
                    } else {
                        debug!("Polling Plaintext stream");
                        FuturePeekStreamImpl::AcceptPlaintext {
                            future: Box::pin(accept.plaintext.accept(stream, service)),
                        }
                    }
                }
                FuturePeekStreamImpl::AcceptTls { future } => {
                    let (stream, service) = ready!(future.poll_unpin(cx))?;
                    return Poll::Ready(Ok((TlsOrPlaintextStream::Tls(stream), service)));
                }
                FuturePeekStreamImpl::AcceptPlaintext { future } => {
                    let (stream, service) = ready!(future.poll_unpin(cx))?;
                    return Poll::Ready(Ok((TlsOrPlaintextStream::Plaintext(stream), service)));
                }
                FuturePeekStreamImpl::Undefined => unreachable!(),
            }
        }
    }
}

#[pin_project]
pub struct PeekStream {
    #[pin]
    read: OwnedReadHalf,
    #[pin]
    write: OwnedWriteHalf,
    #[pin]
    header: Cursor<StreamHeader>,
}

impl tokio::io::AsyncRead for PeekStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.project();
        let () = ready!(this.header.poll_read(cx, buf))?;
        let () = ready!(this.read.poll_read(cx, buf))?;
        Poll::Ready(Ok(()))
    }
}

impl tokio::io::AsyncWrite for PeekStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        self.project().write.poll_write(cx, buf)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        self.project().write.poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        self.project().write.poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        self.project().write.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.write.is_write_vectored()
    }
}

#[pin_project(project = TlsOrPlaintextStreamProj)]
pub enum TlsOrPlaintextStream<TLS, PLAIN> {
    Tls(#[pin] TLS),
    Plaintext(#[pin] PLAIN),
}

impl<T: tokio::io::AsyncRead, P: tokio::io::AsyncRead> tokio::io::AsyncRead
    for TlsOrPlaintextStream<T, P>
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.project() {
            TlsOrPlaintextStreamProj::Tls(s) => s.poll_read(cx, buf),
            TlsOrPlaintextStreamProj::Plaintext(s) => s.poll_read(cx, buf),
        }
    }
}
impl<T: tokio::io::AsyncWrite, P: tokio::io::AsyncWrite> tokio::io::AsyncWrite
    for TlsOrPlaintextStream<T, P>
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        match self.project() {
            TlsOrPlaintextStreamProj::Tls(s) => s.poll_write(cx, buf),
            TlsOrPlaintextStreamProj::Plaintext(s) => s.poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match self.project() {
            TlsOrPlaintextStreamProj::Tls(s) => s.poll_flush(cx),
            TlsOrPlaintextStreamProj::Plaintext(s) => s.poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        match self.project() {
            TlsOrPlaintextStreamProj::Tls(s) => s.poll_shutdown(cx),
            TlsOrPlaintextStreamProj::Plaintext(s) => s.poll_shutdown(cx),
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        match self.project() {
            TlsOrPlaintextStreamProj::Tls(s) => s.poll_write_vectored(cx, bufs),
            TlsOrPlaintextStreamProj::Plaintext(s) => s.poll_write_vectored(cx, bufs),
        }
    }

    fn is_write_vectored(&self) -> bool {
        match self {
            TlsOrPlaintextStream::Tls(s) => s.is_write_vectored(),
            TlsOrPlaintextStream::Plaintext(s) => s.is_write_vectored(),
        }
    }
}
