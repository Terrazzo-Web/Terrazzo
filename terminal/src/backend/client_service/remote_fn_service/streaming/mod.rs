//! Forward [server_fn] calls to mesh clients.

use std::collections::HashMap;
use std::sync::OnceLock;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use server_fn::ServerFnError;

use self::remote_fn::RegisteredRemoteFn;
use super::RemoteFnServerError;
use crate::backend::client_service::routing::DistributedCallbackError;

mod callback;
mod dispatch;
mod grpc;
pub mod remote_fn;
mod response;
pub mod uplift;

inventory::collect!(RegisteredRemoteFn);

/// The collection of remote functions, declared using the [::inventory] crate.
static REMOTE_FNS: OnceLock<HashMap<&'static str, RegisteredRemoteFn>> = OnceLock::new();

/// Initialize the server and the list of remote functions.
pub fn setup() {
    let mut map: HashMap<&'static str, RegisteredRemoteFn> = HashMap::new();
    for remote_server_fn in inventory::iter::<RegisteredRemoteFn> {
        let old = map.insert(remote_server_fn.name, *remote_server_fn);
        assert! { old.is_none(), "Duplicate RemoteFn: {}", old.unwrap().name };
    }
    let Ok(()) = REMOTE_FNS.set(map) else {
        panic!("REMOTE_SERVER_FNS was already set");
    };
}

#[cfg(test)]
#[allow(unused)]
pub fn setup_for_tests() {
    if REMOTE_FNS.get().is_some() {
        return;
    }
    setup();
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum RemoteFnError {
    #[error("[{n}] REMOTE_FNS was not set", n = self.name())]
    RemoteFnsNotSet,

    #[error("[{n}] The RemoteFn was not found: {0}", n = self.name())]
    RemoteFnNotFound(String),

    #[error("[{n}] {0}", n = self.name())]
    RemoteFnServer(#[from] RemoteFnServerError),

    #[error("[{n}] {0}", n = self.name())]
    Status(tonic::Status),

    #[error("[{n}] {0}", n = self.name())]
    ServerFn(ServerFnError),

    #[error("[{n}] Failed to serialize request: {0}", n = self.name())]
    SerializeRequest(serde_json::Error),

    #[error("[{n}] Failed to deserialize request: {0}", n = self.name())]
    DeserializeRequest(serde_json::Error, String),

    #[error("[{n}] Failed to serialize response: {0}", n = self.name())]
    SerializeResponse(serde_json::Error),

    #[error("[{n}] Failed to deserialize response: {0}, json='{1}'", n = self.name())]
    DeserializeResponse(serde_json::Error, String),

    #[error("[{n}] {0}", n = self.name())]
    Distributed(#[from] Box<DistributedCallbackError<RemoteFnError, tonic::Status>>),
}

/// Convert Remote Server function errors into gRPC status.
mod remote_fn_errors_to_status {
    use super::RemoteFnError;
    use crate::backend::client_service::routing::DistributedCallbackError;

    impl From<RemoteFnError> for tonic::Status {
        fn from(error: RemoteFnError) -> Self {
            match error {
                RemoteFnError::Distributed(mut error) => std::mem::replace(
                    error.as_mut(),
                    DistributedCallbackError::LocalError(RemoteFnError::RemoteFnsNotSet),
                )
                .into(),
                RemoteFnError::RemoteFnsNotSet => tonic::Status::internal(error.to_string()),
                RemoteFnError::RemoteFnServer(error) => tonic::Status::internal(error.to_string()),
                RemoteFnError::RemoteFnNotFound { .. } => {
                    tonic::Status::not_found(error.to_string())
                }
                RemoteFnError::Status(status) => status,
                RemoteFnError::ServerFn(error) => tonic::Status::internal(error.to_string()),
                RemoteFnError::SerializeRequest { .. }
                | RemoteFnError::DeserializeRequest { .. }
                | RemoteFnError::SerializeResponse { .. }
                | RemoteFnError::DeserializeResponse { .. } => {
                    tonic::Status::invalid_argument(error.to_string())
                }
            }
        }
    }
}

macro_rules! declare_remote_fn {
    (
        $(#[$meta:meta])*
        $remote_fn:ident,
        $remote_fn_name:expr,
        $input:ty,
        $output:ty,
        $implem:expr
    ) => {
        $(#[$meta])*
        pub static $remote_fn: remote_fn_service::streaming::remote_fn::RemoteFn<$input, $output> = {
            fn callback(
                server: &std::sync::Arc<crate::backend::Server>,
                arg: &str,
            ) -> remote_fn_service::streaming::remote_fn::RemoteFnResult {
                let callback = remote_fn_service::streaming::uplift::uplift::<$input, _, $output, _>($implem);
                Box::pin(callback(server, arg))
            }

            static REMOTE_FN_REGISTRATION: remote_fn_service::streaming::remote_fn::RegisteredRemoteFn =
                remote_fn_service::streaming::remote_fn::RegisteredRemoteFn::new($remote_fn_name, callback);
            inventory::submit! { REMOTE_FN_REGISTRATION };
            remote_fn_service::streaming::remote_fn::RemoteFn::new(REMOTE_FN_REGISTRATION)
        };
    };
}

pub(crate) use declare_remote_fn;
