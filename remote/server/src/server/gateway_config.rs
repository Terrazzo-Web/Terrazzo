use trz_gateway_common::is_global::IsGlobal;
use trz_gateway_common::security_configuration::HasSecurityConfig;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;

use self::app_config::AppConfig;

pub mod app_config;
mod arc;
pub mod memoize;

pub trait GatewayConfig: IsGlobal {
    fn enable_tracing(&self) -> bool {
        true
    }

    fn host(&self) -> &str {
        "127.0.0.1"
    }

    fn port(&self) -> u16 {
        if cfg!(debug_assertions) { 3000 } else { 3001 }
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

    fn app_config(&self) -> impl AppConfig {
        |router| router
    }
}
