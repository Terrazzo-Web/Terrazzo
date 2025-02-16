use std::sync::Arc;

use trz_gateway_common::id::ClientId;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificate;
use trz_gateway_server::server::gateway_config::GatewayConfig;

use super::test_client_config::TestClientConfig;
use crate::client_config::ClientConfig;
use crate::tunnel_config::TunnelConfig;

#[derive(Debug)]
pub struct TestTunnelConfig<G> {
    client_config: Arc<TestClientConfig<G>>,
    client_certificate: Arc<PemCertificate>,
}

impl<G> TestTunnelConfig<G> {
    pub fn new(
        client_config: Arc<TestClientConfig<G>>,
        client_certificate: Arc<PemCertificate>,
    ) -> Self {
        Self {
            client_config,
            client_certificate,
        }
    }
}

impl<G: GatewayConfig> ClientConfig for TestTunnelConfig<G> {
    fn base_url(&self) -> impl std::fmt::Display {
        self.client_config.base_url()
    }

    fn client_id(&self) -> ClientId {
        self.client_config.client_id()
    }

    type GatewayPki = <TestClientConfig<G> as ClientConfig>::GatewayPki;
    fn gateway_pki(&self) -> Self::GatewayPki {
        self.client_config.gateway_pki()
    }
}

impl<G: GatewayConfig> TunnelConfig for TestTunnelConfig<G> {
    type ClientCertificate = Arc<PemCertificate>;
    fn client_certificate(&self) -> Self::ClientCertificate {
        self.client_certificate.clone()
    }
}
