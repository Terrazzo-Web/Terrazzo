use std::sync::Arc;

use trz_gateway_common::id::ClientName;
use trz_gateway_server::server::gateway_config::GatewayConfig;

use super::test_gateway_config::TestGatewayConfig;
use crate::client::config::ClientConfig;

#[derive(Debug)]
pub struct TestClientConfig {
    base_url: String,
    gateway_config: Arc<TestGatewayConfig>,
    client_name: ClientName,
}

impl TestClientConfig {
    pub fn new(
        gateway_config: Arc<TestGatewayConfig>,
        base_url: String,
        client_name: ClientName,
    ) -> Self {
        Self {
            base_url,
            gateway_config,
            client_name,
        }
    }
}

impl ClientConfig for TestClientConfig {
    fn base_url(&self) -> impl std::fmt::Display {
        self.base_url.clone()
    }

    fn client_name(&self) -> ClientName {
        self.client_name.clone()
    }

    type GatewayPki = <Arc<TestGatewayConfig> as GatewayConfig>::RootCaConfig;
    fn gateway_pki(&self) -> Self::GatewayPki {
        self.gateway_config.root_ca()
    }
}
