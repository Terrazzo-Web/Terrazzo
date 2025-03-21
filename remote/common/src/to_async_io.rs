use std::future::ready;
use std::io::ErrorKind;

use bytes::Bytes;
use futures::Sink;
use futures::SinkExt as _;
use futures::Stream;
use futures::StreamExt as _;
use nameth::NamedType as _;
use nameth::nameth;
use tokio::sync::oneshot;
use tokio_util::io::CopyToBytes;
use tokio_util::io::SinkWriter;
use tokio_util::io::StreamReader;

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
        impl Future<Output = std::io::Result<()>>,
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
    impl Future<Output = std::io::Result<()>>,
) {
    let (error_tx, error_rx) = oneshot::channel();
    let mut error_tx = Some(error_tx);
    let (sink, stream) = web_socket.split();

    let reader = {
        StreamReader::new(stream.map(move |message| {
            let message = message.map(IO::into_data).map_err(|error: IO::Error| {
                let error = std::io::Error::new(ErrorKind::ConnectionAborted, ReadError(error));
                let error_tx = error_tx.take();
                error_tx.map(|error_tx| error_tx.send(error));
                return ErrorKind::ConnectionAborted;
            });
            return message;
        }))
    };

    let writer = {
        let sink = CopyToBytes::new(sink.with(|data| ready(Ok(IO::into_messsge(data)))))
            .sink_map_err(|error: IO::Error| {
                std::io::Error::new(ErrorKind::ConnectionAborted, WriteError(error))
            });
        SinkWriter::new(sink)
    };

    let eos = Box::pin(async {
        match error_rx.await {
            // The stream raised an error.
            Ok(error) => Err(error),

            // The stream was dropped, finished without raising an error.
            Err(oneshot::error::RecvError { .. }) => Ok(()),
        }
    });
    (tokio::io::join(reader, writer), eos)
}
