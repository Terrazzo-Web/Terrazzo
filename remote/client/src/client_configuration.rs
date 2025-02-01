use std::sync::Arc;

use trz_gateway_common::is_configuration::IsConfiguration;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;

pub trait ClientConfig: IsConfiguration {
    fn enable_tracing(&self) -> bool {
        true
    }

    fn host(&self) -> &str {
        "127.0.0.1"
    }

    fn port(&self) -> u16 {
        if cfg!(debug_assertions) {
            3000
        } else {
            3001
        }
    }

    // The issuer of GatewayConfig::TlsConfig
    type ServerPkiConfig: TrustedStoreConfig;
    fn server_pki(&self) -> Self::ServerPkiConfig;

    /// The TLS certificate issued by the Gateway.
    type TlsConfig: CertificateConfig;
    fn tls(&self) -> Self::TlsConfig;
}

impl<T: ClientConfig> ClientConfig for Arc<T> {
    fn enable_tracing(&self) -> bool {
        self.as_ref().enable_tracing()
    }
    fn host(&self) -> &str {
        self.as_ref().host()
    }
    fn port(&self) -> u16 {
        self.as_ref().port()
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
