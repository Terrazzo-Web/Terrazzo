use std::future::ready;
use std::io::ErrorKind;

use futures::Sink;
use futures::SinkExt as _;
use futures::Stream;
use futures::StreamExt as _;
use nameth::NamedType as _;
use nameth::nameth;
use tokio_tungstenite::tungstenite;
use tokio_util::io::CopyToBytes;
use tokio_util::io::SinkWriter;
use tokio_util::io::StreamReader;

pub fn to_async_io(
    web_socket: impl Stream<Item = Result<tungstenite::Message, tungstenite::Error>>
    + Sink<tungstenite::Message, Error = tungstenite::Error>,
) -> impl tokio::io::AsyncRead + tokio::io::AsyncWrite {
    let (sink, stream) = web_socket.split();

    let reader = {
        #[nameth]
        #[derive(thiserror::Error, Debug)]
        #[error("[{n}] {0}", n = Self::type_name())]
        struct ReadError(tungstenite::Error);

        StreamReader::new(stream.map(|message| {
            message
                .map(tungstenite::Message::into_data)
                .map_err(|error| {
                    std::io::Error::new(ErrorKind::ConnectionAborted, ReadError(error))
                })
        }))
    };

    let writer = {
        #[nameth]
        #[derive(thiserror::Error, Debug)]
        #[error("[{n}] {0}", n = Self::type_name())]
        struct WriteError(tungstenite::Error);

        let sink =
            CopyToBytes::new(sink.with(|data| ready(Ok(tungstenite::Message::Binary(data)))))
                .sink_map_err(|error| {
                    std::io::Error::new(ErrorKind::ConnectionAborted, WriteError(error))
                });
        SinkWriter::new(sink)
    };

    tokio::io::join(reader, writer)
}
