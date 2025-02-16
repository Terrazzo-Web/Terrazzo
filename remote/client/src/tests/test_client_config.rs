use trz_gateway_server::server::gateway_config::GatewayConfig;

use crate::client_config::ClientConfig;

#[derive(Debug)]
pub struct TestClientConfig<G> {
    gateway_config: G,
}

impl<G: GatewayConfig> TestClientConfig<G> {
    pub fn new(gateway_config: G) -> Self {
        Self { gateway_config }
    }
}

impl<G: GatewayConfig> ClientConfig for TestClientConfig<G> {
    fn base_url(&self) -> impl std::fmt::Display {
        std::format!("https://localhost:{}", self.gateway_config.port())
    }

    type GatewayPki = G::RootCaConfig;
    fn gateway_pki(&self) -> Self::GatewayPki {
        self.gateway_config.root_ca()
    }
}
