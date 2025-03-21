use std::future::ready;
use std::io::ErrorKind;

use bytes::Bytes;
use futures::Sink;
use futures::SinkExt as _;
use futures::Stream;
use futures::StreamExt as _;
use futures::stream::Once;
use nameth::NamedType as _;
use nameth::nameth;
use tokio::sync::oneshot;
use tokio_util::io::CopyToBytes;
use tokio_util::io::SinkWriter;
use tokio_util::io::StreamReader;
use tracing::info;

/// Helper to convert
/// - an object implementing [Stream] + [Sink]
/// - into an object implementing [tokio::io::AsyncRead] + [tokio::io::AsyncWrite]
pub trait WebSocketIo {
    type Message;
    type Error: std::error::Error + Send + Sync + 'static;

    fn into_data(message: Self::Message) -> Bytes;
    fn into_messsge(bytes: Bytes) -> Self::Message;

    fn to_async_io(
        web_socket: impl Stream<Item = Result<Self::Message, Self::Error>>
        + Sink<Self::Message, Error = Self::Error>,
    ) -> (
        impl tokio::io::AsyncRead + tokio::io::AsyncWrite,
        impl Future<Output = ()>,
    )
    where
        Self: Sized,
    {
        to_async_io_impl::<Self>(web_socket)
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
#[error("[{n}] {0}", n = Self::type_name())]
struct ReadError<E>(E);

#[nameth]
#[derive(thiserror::Error, Debug)]
#[error("[{n}] {0}", n = Self::type_name())]
struct WriteError<E>(E);

fn to_async_io_impl<IO: WebSocketIo>(
    web_socket: impl Stream<Item = Result<IO::Message, IO::Error>>
    + Sink<IO::Message, Error = IO::Error>,
) -> (
    impl tokio::io::AsyncRead + tokio::io::AsyncWrite,
    impl Future<Output = ()>,
) {
    let (signal_tx, signal_rx) = oneshot::channel();
    let (sink, stream) = web_socket.split();

    let reader = {
        StreamReader::new(
            stream
                .map(|message| {
                    message.map(IO::into_data).map_err(|error: IO::Error| {
                        std::io::Error::new(ErrorKind::ConnectionAborted, ReadError(error))
                    })
                })
                .chain(end_of_stream(signal_tx)),
        )
    };

    let writer = {
        let sink = CopyToBytes::new(sink.with(|data| ready(Ok(IO::into_messsge(data)))))
            .sink_map_err(|error: IO::Error| {
                std::io::Error::new(ErrorKind::ConnectionAborted, WriteError(error))
            });
        SinkWriter::new(sink)
    };

    let eos = Box::pin(async {
        let _ = signal_rx.await;
    });
    (tokio::io::join(reader, writer), eos)
}

fn end_of_stream(
    signal: oneshot::Sender<()>,
) -> Once<impl Future<Output = Result<bytes::Bytes, std::io::Error>>> {
    futures::stream::once(Box::pin(async {
        info!("EOS");
        let _ = signal.send(());
        Ok(bytes::Bytes::new())
    }))
}
