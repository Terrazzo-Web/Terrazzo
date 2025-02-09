use std::future::ready;
use std::io::ErrorKind;
use std::pin::Pin;
use std::task::Poll;

use futures::SinkExt as _;
use futures::StreamExt as _;
use nameth::nameth;
use nameth::NamedEnumValues as _;
use nameth::NamedType as _;
use pin_project::pin_project;
use tokio::sync::oneshot;
use tokio_rustls::server::TlsStream;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::io::CopyToBytes;
use tokio_util::io::SinkWriter;
use tokio_util::io::StreamReader;
use tonic::transport::server::Connected;
use tonic::transport::Server;
use tracing::debug;
use tracing::info;
use trz_gateway_common::protos::terrazzo::remote::health::health_service_server::HealthServiceServer;

use super::health::HealthServiceImpl;

impl super::Client {
    pub async fn connect<F>(
        &self,
        shutdown: F,
        terminated_tx: oneshot::Sender<Result<(), TunnelError>>,
    ) -> Result<(), ConnectError>
    where
        F: std::future::Future<Output = ()> + Send + Sync + 'static,
    {
        info!("Connecting WebSocket to {}", self.uri);
        let web_socket_config = None;
        let disable_nagle = true;

        let (web_socket, response) = tokio_tungstenite::connect_async_tls_with_config(
            &self.uri,
            web_socket_config,
            disable_nagle,
            Some(self.tls_client.clone()),
        )
        .await?;
        info!("Connected WebSocket");
        debug!("WebSocket response: {response:?}");

        let (sink, stream) = web_socket.split();

        let reader = {
            #[nameth]
            #[derive(thiserror::Error, Debug)]
            #[error("[{n}] {0}", n = Self::type_name())]
            struct ReadError(tungstenite::Error);

            StreamReader::new(stream.map(|message| {
                message.map(Message::into_data).map_err(|error| {
                    std::io::Error::new(ErrorKind::ConnectionAborted, ReadError(error))
                })
            }))
        };

        let writer = {
            #[nameth]
            #[derive(thiserror::Error, Debug)]
            #[error("[{n}] {0}", n = Self::type_name())]
            struct WriteError(tungstenite::Error);

            let sink = CopyToBytes::new(sink.with(|data| ready(Ok(Message::Binary(data)))))
                .sink_map_err(|error| {
                    std::io::Error::new(ErrorKind::ConnectionAborted, WriteError(error))
                });
            SinkWriter::new(sink)
        };

        let stream = tokio::io::join(reader, writer);

        let tls_stream = self
            .tls_server
            .accept(stream)
            .await
            .map_err(ConnectError::Accept)?;

        let connection = Connection(tls_stream);
        let connection = futures::stream::once(ready(Ok::<_, std::io::Error>(connection)));

        let grpc_server =
            Server::builder().add_service(HealthServiceServer::new(HealthServiceImpl));
        tokio::spawn(async move {
            let result = grpc_server
                .serve_with_incoming_shutdown(connection, shutdown)
                .await;
            let _ = terminated_tx.send(result.map_err(Into::into));
        });
        Ok(())
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ConnectError {
    #[error("[{n}] {0}", n = self.name())]
    Connect(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("[{n}] {0}", n = self.name())]
    Accept(std::io::Error),
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum TunnelError {
    #[error("[{n}] {0}", n = self.name())]
    TunnelFailure(#[from] tonic::transport::Error),
}

// A wrapper for [TlsStream] that implements [Connected].
#[pin_project]
pub struct Connection<C>(#[pin] TlsStream<C>);

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
        self.project().0.poll_read(cx, buf)
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
        self.project().0.poll_write(cx, buf)
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.project().0.poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.project().0.poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        self.project().0.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }
}
