use crate::backend::config::WatchConfig as _;
use std::sync::Arc;

use terrazzo::autoclone;
use terrazzo::axum::Json;
use terrazzo::axum::Router;
use terrazzo::axum::routing::get;
use trz_gateway_common::id::ClientName;

use crate::api::client_address::ClientAddress;
use crate::backend::Server;
use crate::backend::auth::layer::AuthLayer;
use crate::backend::client_service::shared_service::remotes::list_remotes;
use crate::backend::config::DynConfig;

#[autoclone]
pub fn remotes_routes(
    config: &Arc<DynConfig>,
    auth_config: &tokio::sync::watch::Sender<Arc<crate::backend::config::ServerState>>,
    server: &Arc<Server>,
) -> Router {
    let mesh = config.mesh.clone();
    Router::new()
        .route(
            "/remotes",
            get(|| {
                autoclone!(mesh, server);
                let client_name =
                    mesh.with(|mesh| Some(ClientName::from(mesh.as_ref()?.client_name.as_str())));
                call_list_remotes(client_name, server)
            }),
        )
        .route_layer(AuthLayer {
            auth_config: auth_config.clone(),
        })
}

async fn call_list_remotes(
    my_client_name: Option<ClientName>,
    server: Arc<Server>,
) -> Json<Vec<ClientAddress>> {
    let my_client_name = my_client_name
        .map(|n| vec![n.to_string()])
        .unwrap_or_default();
    let mut remotes = list_remotes(&server, my_client_name).await;
    remotes.sort_by_key(|remote| remote.leaf());
    let remotes = remotes.into_iter().map(ClientAddress::from).collect();
    Json(remotes)
}
