use std::future::ready;
use std::sync::Arc;

use futures::FutureExt;
use futures::StreamExt as _;
use http::header::InvalidHeaderValue;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use tokio_tungstenite::tungstenite;
use tonic::transport::Server;
use tracing::Span;
use tracing::debug;
use tracing::info;
use tracing_futures::Instrument as _;
use trz_gateway_common::id::CLIENT_ID_HEADER;
use trz_gateway_common::id::ClientId;
use trz_gateway_common::protos::terrazzo::remote::health::health_service_server::HealthServiceServer;
use trz_gateway_common::to_async_io::WebSocketIo;

use self::tungstenite::client::IntoClientRequest as _;
use super::connection::Connection;
use super::health::HealthServiceImpl;

impl super::Client {
    /// API to create tunnels to the Terrazzo Gateway.
    pub(super) async fn connect<F>(
        &self,
        client_id: ClientId,
        shutdown: F,
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

        let (stream, eos) = TungsteniteWebSocketIo::to_async_io(web_socket);
        let eos = eos.map(|r| r.map_err(Arc::new)).shared();
        let tls_stream = self
            .tls_server
            .accept(stream)
            .await
            .map_err(ConnectError::Accept)?;

        let connection = Connection::new(tls_stream);
        let eos2 = eos.clone();
        let incoming = futures::stream::once(ready(Ok(connection)))
            .chain(futures::stream::once(async move {
                let () = eos2.await.map_err(ConnectError::Stream)?;
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

        info!("Serving");
        let () = grpc_server
            .serve_with_incoming_shutdown(incoming, shutdown)
            .await?;
        if let Some(eos) = eos.peek().cloned() {
            let () = eos.map_err(ConnectError::Stream)?;
        }
        info!("Done");
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
    Tunnel(#[from] tonic::transport::Error),

    #[error("[{n}] {0}", n = self.name())]
    Stream(Arc<std::io::Error>),

    #[error("[{n}] The client got disconnected", n = self.name())]
    Disconnected,
}

struct TungsteniteWebSocketIo;
impl WebSocketIo for TungsteniteWebSocketIo {
    type Message = tungstenite::Message;
    type Error = tungstenite::Error;

    fn into_data(message: Self::Message) -> tungstenite::Bytes {
        message.into_data()
    }

    fn into_messsge(bytes: tungstenite::Bytes) -> Self::Message {
        tungstenite::Message::Binary(bytes)
    }
}
