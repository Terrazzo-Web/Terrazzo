use std::sync::Arc;

use trz_gateway_common::id::ClientId;
use trz_gateway_server::server::gateway_config::GatewayConfig;

use super::test_client_config::TestClientConfig;
use crate::certificate_config::ClientCertificateConfig;
use crate::client_config::ClientConfig;

#[derive(Debug)]
pub struct TestClientCertificateConfig<G> {
    client_config: Arc<TestClientConfig<G>>,
    client_id: ClientId,
}

impl<G: GatewayConfig> TestClientCertificateConfig<G> {
    pub fn new(client_config: Arc<TestClientConfig<G>>, client_id: ClientId) -> Self {
        Self {
            client_config,
            client_id,
        }
    }
}

impl<G: GatewayConfig> ClientConfig for TestClientCertificateConfig<G> {
    fn base_url(&self) -> impl std::fmt::Display {
        self.client_config.base_url()
    }

    type GatewayPki = <TestClientConfig<G> as ClientConfig>::GatewayPki;
    fn gateway_pki(&self) -> Self::GatewayPki {
        self.client_config.gateway_pki()
    }
}

impl<G: GatewayConfig> ClientCertificateConfig for TestClientCertificateConfig<G> {
    fn client_id(&self) -> ClientId {
        self.client_id.clone()
    }
}
