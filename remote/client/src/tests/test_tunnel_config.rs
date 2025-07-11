use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use trz_gateway_common::id::ClientName;
use trz_gateway_common::protos::terrazzo::remote::tests::test_tunnel_service_server::TestTunnelServiceServer;
use trz_gateway_common::retry_strategy::RetryStrategy;
use trz_gateway_common::security_configuration::certificate::cache::CachedCertificate;
use trz_gateway_server::server::gateway_config::GatewayConfig;

use super::calculator;
use super::test_client_config::TestClientConfig;
use crate::client::AuthCode;
use crate::client::config::ClientConfig;
use crate::client::service::ClientService;
use crate::tunnel_config::TunnelConfig;

#[derive(Debug)]
pub struct TestTunnelConfig<G> {
    client_config: TestClientConfig<G>,
    client_certificate: CachedCertificate,
}

impl<G> TestTunnelConfig<G> {
    pub fn new(client_config: TestClientConfig<G>, client_certificate: CachedCertificate) -> Self {
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

    fn client_name(&self) -> ClientName {
        self.client_config.client_name()
    }

    type GatewayPki = <TestClientConfig<G> as ClientConfig>::GatewayPki;
    fn gateway_pki(&self) -> Self::GatewayPki {
        self.client_config.gateway_pki()
    }
}

impl<G: GatewayConfig> TunnelConfig for TestTunnelConfig<G> {
    type ClientCertificate = CachedCertificate;
    fn client_certificate(&self) -> Self::ClientCertificate {
        self.client_certificate.clone()
    }

    fn client_service(&self) -> impl ClientService {
        |mut server: tonic::transport::Server| {
            server.add_service(TestTunnelServiceServer::new(calculator::Calculator))
        }
    }

    fn retry_strategy(&self) -> RetryStrategy {
        RetryStrategy::from(Duration::from_secs(1)).exponential_backoff(2., Duration::from_secs(60))
    }

    fn current_auth_code(&self) -> Arc<Mutex<AuthCode>> {
        Arc::new(Mutex::new("".into()))
    }
}
