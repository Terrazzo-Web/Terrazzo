#![cfg(feature = "remote-fn")]

use std::sync::Arc;
use std::sync::Weak;

use crate::backend::Server;
use nameth::NamedEnumValues as _;
use nameth::nameth;

use crate::utils::testable_once_lock::TestableOnceLock;

#[cfg(feature = "remote-fn-streaming")]
pub mod streaming;

#[cfg(feature = "remote-fn-unary")]
pub mod unary;

/// Records the current [Server] instance.
///
/// This is necessary because remote functions are static.
static SERVER: TestableOnceLock<Weak<Server>> = TestableOnceLock::new();

pub fn remote_fn_server() -> Result<Arc<Server>, RemoteFnServerError> {
    SERVER
        .get()
        .as_ref()
        .ok_or(RemoteFnServerError::ServerNotSet)?
        .upgrade()
        .ok_or(RemoteFnServerError::ServerWasDropped)
}

pub fn set_remote_fn_server(server: Weak<Server>) {
    SERVER.set(server);
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum RemoteFnServerError {
    #[error("[{n}] The Server instance was not set", n = self.name())]
    ServerNotSet,

    #[error("[{n}] The Server instance was dropped", n = self.name())]
    ServerWasDropped,
}
