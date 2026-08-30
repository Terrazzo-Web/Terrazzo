//! Public WebRTC signaling endpoints.

use std::sync::Arc;

use axum::extract::Path;
use axum::extract::WebSocketUpgrade;
use axum::response::Response;
use trz_gateway_common::id::ClientName;

use super::Server;

mod server;
pub(super) mod signaling;

impl Server {
    pub(super) async fn p2p_register(
        self: Arc<Self>,
        Path(server_name): Path<ClientName>,
        web_socket: WebSocketUpgrade,
    ) -> Response {
        self.p2p_signaling.clone().register(server_name, web_socket)
    }

    pub(super) async fn p2p_connect(
        self: Arc<Self>,
        Path(server_name): Path<ClientName>,
        web_socket: WebSocketUpgrade,
    ) -> Response {
        self.p2p_signaling
            .clone()
            .connect(server_name, web_socket)
            .await
    }
}
