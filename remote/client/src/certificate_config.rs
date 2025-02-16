use std::ffi::OsString;
use std::sync::Arc;
use std::sync::OnceLock;

use tracing::debug;
use trz_gateway_common::id::ClientId;
use uuid::Uuid;

use crate::client_config::ClientConfig;

/// Configuration to obtain to client certificate.
pub trait ClientCertificateConfig: ClientConfig {
    fn client_id(&self) -> ClientId {
        static CLIENT_ID: OnceLock<ClientId> = OnceLock::new();
        fn make_default_hostname() -> ClientId {
            match hostname::get().map(OsString::into_string) {
                Ok(Ok(hostname)) => return hostname.into(),
                Err(error) => debug!("Failed to get the hostname with hostname::get(): {error}"),
                Ok(Err(error)) => debug!("Failed to parse the hostname string: {error:?}"),
            }
            return Uuid::new_v4().to_string().into();
        }

        CLIENT_ID.get_or_init(make_default_hostname).clone()
    }
}

impl<T: ClientCertificateConfig> ClientCertificateConfig for Arc<T> {}
