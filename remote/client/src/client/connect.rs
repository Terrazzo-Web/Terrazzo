use std::future::ready;

use futures::StreamExt as _;
use http::header::InvalidHeaderValue;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite;
use tonic::transport::Server;
use tracing::Span;
use tracing::debug;
use tracing::info;
use tracing_futures::Instrument as _;
use trz_gateway_common::id::CLIENT_ID_HEADER;
use trz_gateway_common::id::ClientId;
use trz_gateway_common::protos::terrazzo::remote::health::health_service_server::HealthServiceServer;

use self::tungstenite::client::IntoClientRequest as _;
use super::connection::Connection;
use super::health::HealthServiceImpl;
use super::to_async_io::to_async_io;

impl super::Client {
    pub async fn connect<F>(
        &self,
        client_id: ClientId,
        shutdown: F,
        terminated: oneshot::Sender<()>,
    ) -> Result<(), ConnectError>
    where
        F: std::future::Future<Output = ()> + Send + Sync + 'static,
    {
        info!(uri = self.uri, "Connecting WebSocket");
        let web_socket_config = None;
        let disable_nagle = true;

        let mut websocket_uri = format!("ws{}", &self.uri[4..]).into_client_request()?;
        websocket_uri
            .headers_mut()
            .append(&CLIENT_ID_HEADER, client_id.as_ref().try_into()?);
        let (web_socket, response) = tokio_tungstenite::connect_async_tls_with_config(
            websocket_uri,
            web_socket_config,
            disable_nagle,
            Some(self.tls_client.clone()),
        )
        .await?;
        info!("Connected WebSocket");
        debug!("WebSocket response: {response:?}");

        let stream = to_async_io(web_socket);

        let tls_stream = self
            .tls_server
            .accept(stream)
            .await
            .map_err(ConnectError::Accept)?;

        let connection = Connection::new(tls_stream);
        let incoming = futures::stream::once(ready(Ok(connection)))
            .chain(futures::stream::once(async move {
                let () = shutdown.await;
                Err(ConnectError::Disconnected)
            }))
            .in_current_span();

        let current_span = Span::current();
        let grpc_server = self
            .client_service
            .configure_service(
                Server::builder()
                    .tcp_keepalive(None)
                    .tcp_nodelay(true)
                    .http2_keepalive_interval(None)
                    .http2_keepalive_timeout(None)
                    .trace_fn(move |_| current_span.clone()),
            )
            .add_service(HealthServiceServer::new(HealthServiceImpl));

        tokio::spawn(
            async move {
                match grpc_server.serve_with_incoming(incoming).await {
                    Ok(()) => info!("Finished"),
                    Err(error) => info!("Failed: {error}"),
                }
                let _ = terminated.send(());
            }
            .in_current_span(),
        );
        Ok(())
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ConnectError {
    #[error("[{n}] {0}", n = self.name())]
    InvalidHeader(#[from] InvalidHeaderValue),

    #[error("[{n}] {0}", n = self.name())]
    Connect(#[from] tungstenite::Error),

    #[error("[{n}] {0}", n = self.name())]
    Accept(std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    TunnelFailure(#[from] tonic::transport::Error),

    #[error("[{n}] The client got disconnected", n = self.name())]
    Disconnected,
}
