use std::future::ready;
use std::sync::Arc;

use autoclone::autoclone;
use axum::routing::get;
use axum::Router;
use tower_http::trace::DefaultMakeSpan;
use tower_http::trace::TraceLayer;

use super::gateway_configuration::GatewayConfig;
use super::Server;

impl<C: GatewayConfig> Server<C> {
    #[autoclone]
    pub(super) fn make_app(self: &Arc<Self>) -> Router {
        let server = self.clone();
        Router::new()
            .route("/status", get(|| ready("UP")))
            .route(
                "/remote/certificate",
                get(move |request| {
                    autoclone!(server);
                    server.get_certificate(request)
                }),
            )
            .route(
                "/remote/tunnel/{client_id}",
                get(move |client_id, web_socket| {
                    autoclone!(server);
                    server.tunnel(client_id, web_socket)
                }),
            )
            .layer(TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::default()))
    }
}
