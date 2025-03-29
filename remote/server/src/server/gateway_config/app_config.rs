use std::sync::Arc;

use axum::Router;
use trz_gateway_common::is_global::IsGlobal;

use crate::server::Server;

/// Configures the routes served by Terrazzo Gateway HTTP server.
pub trait AppConfig: IsGlobal {
    fn configure_app(&self, server: Arc<Server>, router: Router) -> Router;
}

impl<C: Fn(Arc<Server>, Router) -> Router + IsGlobal> AppConfig for C {
    fn configure_app(&self, server: Arc<Server>, router: Router) -> Router {
        self(server, router)
    }
}
