use std::future::ready;
use std::sync::Arc;

use autoclone::autoclone;
use axum::Router;
use axum::routing::get;
use tracing::Instrument as _;
use tracing::Span;

use super::Server;

impl Server {
    #[autoclone]
    pub(super) fn make_app(self: &Arc<Self>, span: Span) -> Router {
        let server = self.clone();
        let router = Router::new()
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
        self.app_config.configure_app(router)
    }
}
