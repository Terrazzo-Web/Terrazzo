use std::sync::Arc;

use trz_gateway_common::id::ClientName;

use crate::backend::Server;

pub mod convert;
pub mod grpc_error;
#[cfg(feature = "text-editor")]
pub mod notify_service;
#[cfg(feature = "port-forward")]
pub mod port_forward_service;
#[cfg(feature = "remote-fn")]
pub mod remote_fn_service;
mod routing;
pub mod shared_service;
#[cfg(feature = "terminal")]
pub mod terminal_service;
#[cfg(feature = "text-editor")]
pub mod text_editor_service;

#[derive(Clone)]
pub struct ClientServiceImpl {
    client_name: ClientName,
    server: Arc<Server>,
}

impl ClientServiceImpl {
    pub fn new(client_name: ClientName, server: Arc<Server>) -> Self {
        Self {
            client_name,
            server,
        }
    }
}
