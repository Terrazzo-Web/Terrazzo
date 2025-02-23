use std::future::Ready;
use std::future::ready;
use std::io::ErrorKind;
use std::sync::Arc;

use axum::extract::Path;
use axum::extract::WebSocketUpgrade;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use axum::http::HeaderMap;
use axum::http::Uri;
use axum::response::IntoResponse;
use futures::SinkExt;
use futures::StreamExt;
use hyper_util::rt::TokioIo;
use nameth::NamedEnumValues as _;
use nameth::NamedType as _;
use nameth::nameth;
use rustls::pki_types::DnsName;
use rustls::pki_types::InvalidDnsNameError;
use rustls::pki_types::ServerName;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio_util::io::CopyToBytes;
use tokio_util::io::SinkWriter;
use tokio_util::io::StreamReader;
use tonic::transport::Channel;
use tracing::Instrument as _;
use tracing::Span;
use tracing::info;
use tracing::info_span;
use tracing::warn;
use trz_gateway_common::id::CLIENT_ID_HEADER;
use trz_gateway_common::id::ClientName;

use super::Server;

impl Server {
    pub async fn tunnel(
        self: Arc<Self>,
        headers: HeaderMap,
        Path(client_name): Path<ClientName>,
        web_socket: WebSocketUpgrade,
    ) -> impl IntoResponse {
        let span = if let Some(Ok(client_id)) = headers
            .get(&CLIENT_ID_HEADER)
            .map(|client_id| client_id.to_str())
        {
            info_span!("Tunnel", %client_name, %client_id)
        } else {
            info_span!("Tunnel", %client_name)
        };
        let _entered = span.clone().entered();
        info!("Incoming tunnel");
        web_socket.on_upgrade(move |web_socket| {
            let _entered = span.clone().entered();
            self.process_websocket(client_name, web_socket);
            ready(())
        })
    }

    fn process_websocket(self: Arc<Self>, client_name: ClientName, web_socket: WebSocket) {
        let (sink, stream) = web_socket.split();

        let reader = {
            #[nameth]
            #[derive(thiserror::Error, Debug)]
            #[error("[{n}] {0}", n = Self::type_name())]
            struct ReadError(axum::Error);

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
            struct WriteError(axum::Error);

            let sink = CopyToBytes::new(sink.with(|data| ready(Ok(Message::Binary(data)))))
                .sink_map_err(|error| {
                    std::io::Error::new(ErrorKind::ConnectionAborted, WriteError(error))
                });
            SinkWriter::new(sink)
        };

        let stream = tokio::io::join(reader, writer);
        tokio::spawn(
            self.process_connection(client_name, stream)
                .in_current_span(),
        );
    }

    async fn process_connection(
        self: Arc<Self>,
        client_name: ClientName,
        connection: impl AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    ) -> Result<(), TunnelError> {
        let tls_stream = self
            .tls_client
            .connect(
                ServerName::DnsName(DnsName::try_from(client_name.to_string())?),
                connection,
            )
            .await
            .map_err(TunnelError::TlsConnectError)?;

        // The endpoint is irrelevant: gRPC isn't actually connecting to this endpoint.
        // Instead we are manually providing the connection using 'connect_with_connector'.
        // The connection used by gRPC is the bi-directional stream based on the WebSocket.
        let endpoint = tonic::transport::Endpoint::new(format!(
            "https://localhost/remote/tunnel/{client_name}"
        ))
        .map_err(|_| TunnelError::InvalidEndpoint)?;
        let connector = make_single_use_connector(tls_stream)
            .await
            .map_err(TunnelError::SingleUseConnectorError)?;
        let channel: Channel = endpoint
            .connect_with_connector(tower::service_fn(connector))
            .await
            .map_err(TunnelError::GrpcConnectError)?;

        self.connections.add(client_name, channel);
        Ok(())
    }
}

async fn make_single_use_connector<S: AsyncRead + AsyncWrite>(
    stream: S,
) -> std::io::Result<impl FnMut(Uri) -> Ready<std::io::Result<TokioIo<S>>>> {
    let span = Span::current();
    let mut single_use_connection = Some(TokioIo::new(stream));
    let connector = move |_uri| {
        span.in_scope(|| {
            let Some(connection) = single_use_connection.take() else {
                let error = std::io::Error::new(
                    ErrorKind::AddrInUse,
                    "The WebSocket was already used to create a channel",
                );
                warn!("{error}");
                return ready(Err(error));
            };
            // `single_use_connection` has been consumed and is now empty.
            assert!(single_use_connection.is_none());
            ready(Ok(connection))
        })
    };
    Ok(connector)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum TunnelError {
    #[error("[{n}] Failed to create synthetic endpoint", n = self.name())]
    InvalidEndpoint,

    #[error("[{n}] {0}", n = self.name())]
    InvalidDnsName(#[from] InvalidDnsNameError),

    #[error("[{n}] {0}", n = self.name())]
    TlsConnectError(std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    SingleUseConnectorError(std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    GrpcConnectError(tonic::transport::Error),
}
