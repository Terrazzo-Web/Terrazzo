use trz_gateway_common::id::ClientId;
use trz_gateway_server::server::gateway_config::GatewayConfig;

use crate::client_config::ClientConfig;

#[derive(Debug)]
pub struct TestClientConfig<G> {
    gateway_config: G,
    client_id: ClientId,
}

impl<G> TestClientConfig<G> {
    pub fn new(gateway_config: G, client_id: ClientId) -> Self {
        Self {
            gateway_config,
            client_id,
        }
    }
}

impl<G: GatewayConfig> ClientConfig for TestClientConfig<G> {
    fn base_url(&self) -> impl std::fmt::Display {
        std::format!("https://localhost:{}", self.gateway_config.port())
    }

    fn client_id(&self) -> ClientId {
        self.client_id.clone()
    }

    type GatewayPki = G::RootCaConfig;
    fn gateway_pki(&self) -> Self::GatewayPki {
        self.gateway_config.root_ca()
    }
}
