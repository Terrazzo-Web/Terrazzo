use std::sync::Arc;

use trz_gateway_common::dynamic_config::has_diff::DiffArc;
use trz_gateway_common::id::ClientName;
use trz_gateway_server::server::Server;

use super::config::DynConfig;

pub mod convert;
pub mod grpc_error;
pub mod notify_service;
pub mod port_forward_service;
pub mod remote_fn_service;
mod routing;
pub mod shared_service;
pub mod terminal_service;

#[derive(Clone)]
pub struct ClientServiceImpl {
    client_name: ClientName,
    server: Arc<Server>,
    config: DiffArc<DynConfig>,
}

impl ClientServiceImpl {
    pub fn new(client_name: ClientName, server: Arc<Server>, config: DiffArc<DynConfig>) -> Self {
        Self {
            client_name,
            server,
            config,
        }
    }
}
