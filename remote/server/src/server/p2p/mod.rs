//! Public WebRTC signaling endpoints.

use std::sync::Arc;

use axum::extract::Path;
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse as _;
use axum::response::Response;
use trz_gateway_common::id::ClientName;

use super::Server;

mod server;
pub(super) mod signaling;

/// WebSocket endpoints for establishing peer-to-peer connections.
///
/// A server first opens the registration endpoint for its name and sends a
/// [`SignalMessage::Hello`]. The resulting WebSocket remains registered so it
/// can negotiate any connections subsequently opened through the connect
/// endpoint. A connecting client likewise sends `Hello`; the signaling server
/// then allocates a connection ID and sends [`SignalMessage::Start`] to both
/// peers.
///
/// For that connection ID, the client sends an SDP offer and the registered
/// server returns an SDP answer using [`SignalMessage::Description`]. Both
/// sides may exchange [`SignalMessage::IceCandidate`] messages followed by
/// [`SignalMessage::EndOfCandidates`]. [`SignalMessage::Cancel`] and
/// [`SignalMessage::Failure`] terminate unsuccessful negotiations. Once WebRTC
/// is established, application traffic travels peer-to-peer rather than over
/// these signaling WebSockets.
///
/// [`SignalMessage::Hello`]: trz_gateway_common::p2p::protocol::SignalMessage::Hello
/// [`SignalMessage::Start`]: trz_gateway_common::p2p::protocol::SignalMessage::Start
/// [`SignalMessage::Description`]: trz_gateway_common::p2p::protocol::SignalMessage::Description
/// [`SignalMessage::IceCandidate`]: trz_gateway_common::p2p::protocol::SignalMessage::IceCandidate
/// [`SignalMessage::EndOfCandidates`]: trz_gateway_common::p2p::protocol::SignalMessage::EndOfCandidates
/// [`SignalMessage::Cancel`]: trz_gateway_common::p2p::protocol::SignalMessage::Cancel
/// [`SignalMessage::Failure`]: trz_gateway_common::p2p::protocol::SignalMessage::Failure
impl Server {
    /// Registers a named server for WebRTC signaling.
    ///
    /// Upgrades the request to a persistent WebSocket and, after validating its
    /// initial `Hello`, makes it available to connecting clients. A newer
    /// registration for the same name replaces the existing one.
    pub(super) async fn p2p_register(
        self: Arc<Self>,
        Path(server_name): Path<ClientName>,
        web_socket: WebSocketUpgrade,
    ) -> Response {
        self.p2p_signaling.clone().register(server_name, web_socket)
    }

    /// Connects a client to a named server for WebRTC signaling.
    ///
    /// Waits briefly for an active registration, allocates a signaling session,
    /// and upgrades the request to the client WebSocket that relays negotiation
    /// messages to and from the registered server.
    pub(super) async fn p2p_connect(
        self: Arc<Self>,
        Path(server_name): Path<ClientName>,
        web_socket: WebSocketUpgrade,
    ) -> Response {
        self.p2p_signaling
            .clone()
            .connect(server_name, web_socket)
            .await
            .unwrap_or_else(|status_code| status_code.into_response())
    }
}
