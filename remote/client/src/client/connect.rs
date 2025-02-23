use std::convert::Infallible;
use std::future::ready;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite;
use tonic::transport::Server;
use tracing::debug;
use tracing::info;
use trz_gateway_common::protos::terrazzo::remote::health::health_service_server::HealthServiceServer;

use super::connection::Connection;
use super::health::HealthServiceImpl;
use super::to_async_io::to_async_io;

impl super::Client {
    pub async fn connect<F>(
        &self,
        shutdown: F,
        terminated: oneshot::Sender<()>,
    ) -> Result<(), ConnectError>
    where
        F: std::future::Future<Output = ()> + Send + Sync + 'static,
    {
        info!("Connecting WebSocket to {}", self.uri);
        let web_socket_config = None;
        let disable_nagle = true;

        let websocket_uri = format!("ws{}", &self.uri[4..]);
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

        use futures::FutureExt;
        let shutdown = shutdown.shared();
        let connection = Connection::new(tls_stream, terminated);
        let incoming = futures::stream::once(ready(Ok::<_, Infallible>(connection)));

        let grpc_server = self
            .client_service
            .configure_service(Server::builder().tcp_keepalive(None).tcp_nodelay(true))
            .add_service(HealthServiceServer::new(HealthServiceImpl));
        let () = grpc_server
            .serve_with_incoming_shutdown(incoming, shutdown)
            .await?;
        Ok(())
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ConnectError {
    #[error("[{n}] {0}", n = self.name())]
    Connect(#[from] tungstenite::Error),

    #[error("[{n}] {0}", n = self.name())]
    Accept(std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    TunnelFailure(#[from] tonic::transport::Error),
}
