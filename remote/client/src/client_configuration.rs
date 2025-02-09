use std::ffi::OsString;
use std::sync::Arc;
use std::sync::OnceLock;

use trz_gateway_common::id::ClientId;
use trz_gateway_common::is_configuration::IsConfiguration;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;
use uuid::Uuid;

pub trait ClientConfig: IsConfiguration {
    fn client_id(&self) -> ClientId {
        static CLIENT_ID: OnceLock<ClientId> = OnceLock::new();
        fn make_default_hostname() -> ClientId {
            if let Ok(Ok(hostname)) = hostname::get().map(OsString::into_string) {
                hostname
            } else {
                Uuid::new_v4().to_string()
            }
            .into()
        }

        CLIENT_ID.get_or_init(make_default_hostname).clone()
    }

    fn base_url(&self) -> impl std::fmt::Display {
        let port = if cfg!(debug_assertions) { 3000 } else { 3001 };
        format!("https://localhost:{port}")
    }

    fn http_client(&self) -> reqwest::Client;

    // The issuer of GatewayConfig::TlsConfig
    type ServerPkiConfig: TrustedStoreConfig;
    fn server_pki(&self) -> Self::ServerPkiConfig;

    /// The TLS certificate issued by the Gateway.
    type TlsConfig: CertificateConfig;
    fn tls(&self) -> Self::TlsConfig;
}

impl<T: ClientConfig> ClientConfig for Arc<T> {
    fn base_url(&self) -> impl std::fmt::Display {
        self.as_ref().base_url()
    }

    fn http_client(&self) -> reqwest::Client {
        self.as_ref().http_client()
    }

    type ServerPkiConfig = T::ServerPkiConfig;
    fn server_pki(&self) -> Self::ServerPkiConfig {
        self.as_ref().server_pki()
    }

    type TlsConfig = T::TlsConfig;
    fn tls(&self) -> Self::TlsConfig {
        self.as_ref().tls()
    }
}
