use std::sync::Arc;

use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;

use super::client_config::ClientConfig;

pub trait TunnelConfig: ClientConfig {
    // The issuer of GatewayConfig::TlsConfig
    type ServerPkiConfig: TrustedStoreConfig;
    fn server_pki(&self) -> Self::ServerPkiConfig;

    /// The TLS certificate issued by the Gateway.
    type TlsConfig: CertificateConfig;
    fn tls(&self) -> Self::TlsConfig;
}

impl<T: TunnelConfig> TunnelConfig for Arc<T> {
    type ServerPkiConfig = T::ServerPkiConfig;
    fn server_pki(&self) -> Self::ServerPkiConfig {
        self.as_ref().server_pki()
    }

    type TlsConfig = T::TlsConfig;
    fn tls(&self) -> Self::TlsConfig {
        self.as_ref().tls()
    }
}
