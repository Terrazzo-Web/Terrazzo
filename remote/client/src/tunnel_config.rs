use std::sync::Arc;

use trz_gateway_common::security_configuration::certificate::CertificateConfig;

use super::client_config::ClientConfig;

pub trait TunnelConfig: ClientConfig {
    /// The TLS certificate issued by the Terrazzo Gateway.
    type ClientCertificate: CertificateConfig;
    fn client_certificate(&self) -> Self::ClientCertificate;
}

impl<T: TunnelConfig> TunnelConfig for Arc<T> {
    type ClientCertificate = T::ClientCertificate;
    fn client_certificate(&self) -> Self::ClientCertificate {
        self.as_ref().client_certificate()
    }
}
