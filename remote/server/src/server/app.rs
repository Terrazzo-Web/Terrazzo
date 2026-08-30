use std::future::ready;
use std::sync::Arc;

use autoclone::autoclone;
use axum::Router;
use axum::routing::get;
use tracing::Instrument as _;
use tracing::Span;

use super::Server;

impl Server {
    /// Configures the Terrazzo Gateway HTTP server.
    ///
    /// By default, the only available routes are
    /// - status
    /// - issue a new client certificate
    /// - connect a WebSocket tunnel
    ///
    /// More routes can be configured using [super::gateway_config::app_config::AppConfig].
    #[autoclone]
    pub(super) fn make_app(self: &Arc<Self>) -> Router {
        let span = Span::current();
        let server = self.clone();
        let router = Router::new()
            /*
            TODO: Create a plan to allow Terrazzo Client and Server to connect over WebRTC instead of requiring the Server to be reachable directly.
            Please put the plan in terminal/plans so I can review.
            - Add a /p2p/... API to this server to allow peers to create direct peer to peer connections with WebRTC.
            - There are 3 kinds of nodes
              - Signaling: trz_gateway_server acting as the signaling server exposing the /p2p/... API. This node runs on the public internet.
              - Server: trz_gateway_server acting as the Terrazzo server from terminal/src/backend/mod.rs. This nodes runs behind a NAT and can be connected p2p via STUN but does not have a public IP. It registers itself with the signaling server.
              - Client: trz_gateway_client connects to the Server via WebRTC. Instead of remote/client/src/client/connect.rs only supporting WebSocket connections, it will need to support WebRTC connections. The WebRTC connection needs to be a reliable bi-directional channel so we can transport http/1.1 or http/2 traffic, not a UDP connection subject to packet loss.
            - Use the https://docs.rs/webrtc crate
            - Servers (as in trz_gateway_server) register using their trz_gateway_common::id::ClientName which is assumed to be globally unique
            - Clients (as in trz_gateway_client) can then connect to a given server by starting the connection handshake with the Signaling server
            - When Client/Server are connected p2p
              - Client needs to connect over the p2p connection instead of a socket. I think we can do this by adding a "layer" to remote/client/src/http_client.rs
              - Server needs to serve this API (I mean this `pub(super) fn make_app(self: &Arc<Self>) -> Router`) on the other end of the WebRTC connection
            - Tests: Use the Google STUN server to add a test to remote/client/src/tests.rs starting a signaling server on a port, a test server on another port unknown to the client, and the client should obtain a certificate from the server through the WebRTC connection
            */
            .route("/status", get(|| ready("UP")))
            .route(
                "/remote/certificate",
                get(move |request| {
                    autoclone!(server, span);
                    server.get_certificate(request).instrument(span)
                }),
            )
            .route(
                "/remote/tunnel/{client_name}",
                get(move |client_id, client_name, web_socket| {
                    autoclone!(server, span);
                    server
                        .tunnel(client_id, client_name, web_socket)
                        .instrument(span)
                }),
            );
        self.app_config.configure_app(self.clone(), router)
    }
}
