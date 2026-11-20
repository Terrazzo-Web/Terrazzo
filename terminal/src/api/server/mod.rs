#![cfg(feature = "server")]

use std::sync::Arc;

use terrazzo::axum::Router;

use crate::api::server::common::login::login_routes;
use crate::api::server::common::remotes::remotes_routes;
use crate::backend::Server;
use crate::backend::config::DynConfig;

mod common;

pub fn api_routes(config: &Arc<DynConfig>, server: &Arc<Server>) -> Router {
    let auth_config = &config.server;
    let router = Router::new()
        .merge(login_routes(config))
        .merge(remotes_routes(config, auth_config, server));

    #[cfg(feature = "text-editor")]
    let router = router.merge(crate::text_editor::fsio::api::fsio_routes(
        auth_config,
        server,
    ));

    return router;
}
