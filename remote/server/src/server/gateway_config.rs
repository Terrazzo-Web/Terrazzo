use trz_gateway_common::is_global::IsGlobal;
use trz_gateway_common::security_configuration::HasSecurityConfig;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;

use self::app_config::AppConfig;

pub mod app_config;
mod arc;

/// Configuration for the Terrazzo Gateway.
pub trait GatewayConfig: IsGlobal {
    /// Whether to enable tracing.
    fn enable_tracing(&self) -> bool {
        true
    }

    /// Host to listen to.
    fn host(&self) -> String {
        "127.0.0.1".into()
    }

    /// Port to listen to.
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

    /// The certificate used to sign and validate the custom extension
    /// of client X509 certificates.
    type ClientCertificateIssuerConfig: HasSecurityConfig;
    fn client_certificate_issuer(&self) -> Self::ClientCertificateIssuerConfig;

    /// Configures the routes served by Terrazzo Gateway HTTP server.
    fn app_config(&self) -> impl AppConfig {
        |_server, router| router
    }
}
