use std::sync::Arc;
use std::sync::Mutex;

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

        let terminated_tx = {
            let terminated_tx = Arc::new(Mutex::new(Some(terminated_tx)));
            move |result| {
                let terminated_tx = terminated_tx.lock().expect("terminated_tx").take();
                if let Some(terminated_tx) = terminated_tx {
                    let _sent = terminated_tx.send(result);
                }
            }
        };

        let connection = Connection::new(tls_stream);
        let connection = futures::stream::iter(
            [Ok(connection), Err(TunnelError::Disconnected)]
                .into_iter()
                .inspect({
                    let terminated_tx = terminated_tx.clone();
                    move |connection| {
                        if connection.is_err() {
                            let _ = terminated_tx(Err(TunnelError::Disconnected));
                        }
                    }
                }),
        );

        let grpc_server =
            Server::builder().add_service(HealthServiceServer::new(HealthServiceImpl));
        tokio::spawn(async move {
            let result = grpc_server
                .serve_with_incoming_shutdown(connection, shutdown)
                .await;
            let _ = terminated_tx(result.map_err(Into::into));
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

    #[error("[{n}] The client got disconnected", n = self.name())]
    Disconnected,
}
