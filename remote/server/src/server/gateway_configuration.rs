use std::sync::Arc;

use crate::security_configuration::SecurityConfig;
use crate::utils::is_configuration::IsConfiguration;

pub trait GatewayConfig: IsConfiguration {
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

    /// The root CA is used to issue the client certificates.
    ///
    /// This asset is never rotated, even if the private key leaks.
    /// Security is based on the signed extension of client certificates.
    type RootCaConfig: SecurityConfig;
    fn root_ca(&self) -> Self::RootCaConfig;

    /// The TLS certificate used to listen to HTTPS connections.
    type TlsConfig: SecurityConfig;
    fn tls(&self) -> Self::TlsConfig;

    /// The certificate used to sign the custom extension of X509 certificates.
    type ClientCertificateIssuerConfig: SecurityConfig;
    fn client_certificate_issuer(&self) -> Self::TlsConfig;
}

impl<T: GatewayConfig> GatewayConfig for Arc<T> {
    fn enable_tracing(&self) -> bool {
        let this: &T = self.as_ref();
        this.enable_tracing()
    }
    fn host(&self) -> &str {
        let this: &T = self.as_ref();
        this.host()
    }
    fn port(&self) -> u16 {
        let this: &T = self.as_ref();
        this.port()
    }

    type RootCaConfig = T::RootCaConfig;
    fn root_ca(&self) -> Self::RootCaConfig {
        let this: &T = self.as_ref();
        this.root_ca()
    }

    type TlsConfig = T::TlsConfig;
    fn tls(&self) -> Self::TlsConfig {
        let this: &T = self.as_ref();
        this.tls()
    }

    type ClientCertificateIssuerConfig = T::ClientCertificateIssuerConfig;
    fn client_certificate_issuer(&self) -> Self::TlsConfig {
        let this: &T = self.as_ref();
        this.client_certificate_issuer()
    }
}
