use trz_gateway_common::id::ClientName;
use trz_gateway_server::server::gateway_config::GatewayConfig;

use crate::client_config::ClientConfig;

#[derive(Debug)]
pub struct TestClientConfig<G> {
    gateway_config: G,
    client_name: ClientName,
}

impl<G> TestClientConfig<G> {
    pub fn new(gateway_config: G, client_name: ClientName) -> Self {
        Self {
            gateway_config,
            client_name,
        }
    }
}

impl<G: GatewayConfig> ClientConfig for TestClientConfig<G> {
    fn base_url(&self) -> impl std::fmt::Display {
        std::format!("https://localhost:{}", self.gateway_config.port())
    }

    fn client_name(&self) -> ClientName {
        self.client_name.clone()
    }

    type GatewayPki = G::RootCaConfig;
    fn gateway_pki(&self) -> Self::GatewayPki {
        self.gateway_config.root_ca()
    }
}
