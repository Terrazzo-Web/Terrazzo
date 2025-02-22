use std::sync::Arc;

use trz_gateway_common::is_global::IsGlobal;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::security_configuration::HasSecurityConfig;

pub trait GatewayConfig: IsGlobal {
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
    type RootCaConfig: HasSecurityConfig;
    fn root_ca(&self) -> Self::RootCaConfig;

    /// The TLS certificate used to listen to HTTPS connections.
    type TlsConfig: CertificateConfig;
    fn tls(&self) -> Self::TlsConfig;

    /// The certificate used to sign the custom extension of X509 certificates.
    type ClientCertificateIssuerConfig: HasSecurityConfig;
    fn client_certificate_issuer(&self) -> Self::ClientCertificateIssuerConfig;
}

impl<T: GatewayConfig> GatewayConfig for Arc<T> {
    fn enable_tracing(&self) -> bool {
        self.as_ref().enable_tracing()
    }
    fn host(&self) -> &str {
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
}
