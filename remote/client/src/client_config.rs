use std::sync::Arc;

use trz_gateway_common::is_configuration::IsConfiguration;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;

/// Configuration for the Terrazzo client.
///
/// This is used to:
/// 1. Securely fetch and cache the client certificate,
/// 2. Securely connect to the Terrazzo Gateway.
///
/// Both cases require the base URL to connect to and the PKI to trust.
///
/// Used by
/// - [ClientCertificateConfig] to initialize a certificate for a client_id
/// - [TunnelConfig] to create tunnels using the certificate obtained from [ClientCertificateConfig]
///
/// [ClientCertificateConfig]: crate::certificate_config::ClientCertificateConfig
/// [TunnelConfig]: crate::tunnel_config::TunnelConfig
pub trait ClientConfig: IsConfiguration {
    /// The URL where the Terrazzo Gateway is listening.
    fn base_url(&self) -> impl std::fmt::Display {
        let port = if cfg!(debug_assertions) { 3000 } else { 3001 };
        format!("https://localhost:{port}")
    }

    type GatewayPki: TrustedStoreConfig;

    /// The trust anchors for the Terrazzo Gateway server certificate.
    fn gateway_pki(&self) -> Self::GatewayPki;
}

impl<T: ClientConfig> ClientConfig for Arc<T> {
    fn base_url(&self) -> impl std::fmt::Display {
        self.as_ref().base_url()
    }

    type GatewayPki = T::GatewayPki;
    fn gateway_pki(&self) -> Self::GatewayPki {
        self.as_ref().gateway_pki()
    }
}
