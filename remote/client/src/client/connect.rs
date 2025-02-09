use std::future::ready;

use nameth::nameth;
use nameth::NamedEnumValues as _;
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

        let stream = to_async_io(web_socket);

        let tls_stream = self
            .tls_server
            .accept(stream)
            .await
            .map_err(ConnectError::Accept)?;

        let connection = Connection::new(tls_stream);
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
    Connect(#[from] tungstenite::Error),

    #[error("[{n}] {0}", n = self.name())]
    Accept(std::io::Error),
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum TunnelError {
    #[error("[{n}] {0}", n = self.name())]
    TunnelFailure(#[from] tonic::transport::Error),
}
