use axum::Router;
use trz_gateway_common::is_global::IsGlobal;

/// Configures the routes served by Terrazzo Gateway HTTP server.
pub trait AppConfig: IsGlobal {
    fn configure_app(&self, router: Router) -> Router;
}

impl<C: Fn(Router) -> Router + IsGlobal> AppConfig for C {
    fn configure_app(&self, router: Router) -> Router {
        self(router)
    }
}
