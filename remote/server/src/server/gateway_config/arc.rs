use std::sync::Arc;

use super::GatewayConfig;
use super::app_config::AppConfig;

impl<T: GatewayConfig> GatewayConfig for Arc<T> {
    fn enable_tracing(&self) -> bool {
        self.as_ref().enable_tracing()
    }
    fn host(&self) -> String {
        self.as_ref().host()
    }
    fn port(&self) -> u16 {
        self.as_ref().port()
    }

    type RootCaConfig = T::RootCaConfig;
    fn root_ca(&self) -> Self::RootCaConfig {
        self.as_ref().root_ca()
    }

    type TlsConfig = T::TlsConfig;
    fn tls(&self) -> Self::TlsConfig {
        self.as_ref().tls()
    }

    type ClientCertificateIssuerConfig = T::ClientCertificateIssuerConfig;
    fn client_certificate_issuer(&self) -> Self::ClientCertificateIssuerConfig {
        self.as_ref().client_certificate_issuer()
    }

    fn app_config(&self) -> impl AppConfig {
        self.as_ref().app_config()
    }
}
