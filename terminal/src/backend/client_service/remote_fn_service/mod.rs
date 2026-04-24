use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::Weak;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use trz_gateway_server::server::Server;

pub mod streaming;
pub mod unary;

/// Records the current [Server] instance.
///
/// This is necessary because remote functions are static.
static SERVER: OnceLock<Weak<Server>> = OnceLock::new();

pub fn remote_fn_server() -> Result<Arc<Server>, RemoteFnServerError> {
    let server = SERVER.get().ok_or(RemoteFnServerError::ServerNotSet)?;
    server
        .upgrade()
        .ok_or(RemoteFnServerError::ServerWasDropped)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum RemoteFnServerError {
    #[error("[{n}] The Server instance was not set", n = self.name())]
    ServerNotSet,

    #[error("[{n}] The Server instance was dropped", n = self.name())]
    ServerWasDropped,
}
