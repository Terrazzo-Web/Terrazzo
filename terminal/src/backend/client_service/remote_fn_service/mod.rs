use std::sync::Arc;
use std::sync::Weak;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use trz_gateway_server::server::Server;

pub mod streaming;
pub mod unary;

/// Records the current [Server] instance.
///
/// This is necessary because remote functions are static.
#[cfg(not(test))]
static SERVER: std::sync::OnceLock<Weak<Server>> = std::sync::OnceLock::new();

#[cfg(test)]
static SERVER: std::sync::Mutex<Weak<Server>> = std::sync::Mutex::new(Weak::new());

pub fn remote_fn_server() -> Result<Arc<Server>, RemoteFnServerError> {
    #[cfg(not(test))]
    let server = SERVER.get().ok_or(RemoteFnServerError::ServerNotSet)?;

    #[cfg(test)]
    let server: std::sync::MutexGuard<'_, Weak<Server>> = remote_server_fn_for_tests();

    server
        .upgrade()
        .ok_or(RemoteFnServerError::ServerWasDropped)
}

fn set_remote_fn_server(server: &Arc<Server>) {
    #[cfg(not(test))]
    {
        SERVER.set(Arc::downgrade(server)).unwrap();
    }

    #[cfg(test)]
    {
        *remote_server_fn_for_tests() = Arc::downgrade(server);
    }
}

#[cfg(test)]
pub fn remote_server_fn_for_tests() -> std::sync::MutexGuard<'static, Weak<Server>> {
    SERVER.lock().expect("SERVER")
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum RemoteFnServerError {
    #[error("[{n}] The Server instance was not set", n = self.name())]
    ServerNotSet,

    #[error("[{n}] The Server instance was dropped", n = self.name())]
    ServerWasDropped,
}
