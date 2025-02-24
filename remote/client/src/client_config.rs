use std::ffi::OsString;
use std::sync::Arc;
use std::sync::OnceLock;

use tracing::debug;
use trz_gateway_common::id::ClientName;
use trz_gateway_common::is_global::IsGlobal;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;
use uuid::Uuid;

/// Configuration for the Terrazzo client.
///
/// This is used to:
/// 1. Securely fetch and cache the client certificate,
/// 2. Securely connect to the Terrazzo Gateway.
///
/// Both cases require the base URL to connect to and the PKI to trust.
///
/// Used by [TunnelConfig] to create tunnels using the certificate obtained from [ClientConfig]
///
/// [TunnelConfig]: crate::tunnel_config::TunnelConfig
pub trait ClientConfig: IsGlobal {
    /// The URL where the Terrazzo Gateway is listening.
    fn base_url(&self) -> impl std::fmt::Display {
        let port = if cfg!(debug_assertions) { 3000 } else { 3001 };
        format!("https://localhost:{port}")
    }

    fn client_name(&self) -> ClientName {
        static CLIENT_ID: OnceLock<ClientName> = OnceLock::new();
        fn make_default_hostname() -> ClientName {
            match hostname::get().map(OsString::into_string) {
                Ok(Ok(hostname)) => return hostname.into(),
                Err(error) => debug!("Failed to get the hostname with hostname::get(): {error}"),
                Ok(Err(error)) => debug!("Failed to parse the hostname string: {error:?}"),
            }
            return Uuid::new_v4().to_string().into();
        }

        CLIENT_ID.get_or_init(make_default_hostname).clone()
    }

    type GatewayPki: TrustedStoreConfig;

    /// The trust anchors for the Terrazzo Gateway server certificate.
    fn gateway_pki(&self) -> Self::GatewayPki;
}

impl<T: ClientConfig> ClientConfig for Arc<T> {
    fn base_url(&self) -> impl std::fmt::Display {
        self.as_ref().base_url()
    }

    fn client_name(&self) -> ClientName {
        self.as_ref().client_name()
    }

    type GatewayPki = T::GatewayPki;
    fn gateway_pki(&self) -> Self::GatewayPki {
        self.as_ref().gateway_pki()
    }
}
