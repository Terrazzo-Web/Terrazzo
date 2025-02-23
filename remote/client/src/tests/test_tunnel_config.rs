use std::sync::Arc;

use trz_gateway_common::id::ClientName;
use trz_gateway_common::protos::terrazzo::remote::tests::test_tunnel_service_server::TestTunnelServiceServer;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificate;
use trz_gateway_server::server::gateway_config::GatewayConfig;

use super::test_client_config::TestClientConfig;
use crate::client_config::ClientConfig;
use crate::client_service::ClientService;
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

    fn client_name(&self) -> ClientName {
        self.client_config.client_name()
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

    fn client_service(&self) -> impl ClientService {
        |mut server: tonic::transport::Server| {
            server.add_service(TestTunnelServiceServer::new(calculator::Calculator))
        }
    }
}

mod calculator {
    use tonic::Status;
    use tonic::async_trait;
    use tracing::info;
    use trz_gateway_common::protos::terrazzo::remote::tests::Expression;
    use trz_gateway_common::protos::terrazzo::remote::tests::Operator;
    use trz_gateway_common::protos::terrazzo::remote::tests::Value;
    use trz_gateway_common::protos::terrazzo::remote::tests::expression;
    use trz_gateway_common::protos::terrazzo::remote::tests::test_tunnel_service_server::TestTunnelService;
    use trz_gateway_common::protos::terrazzo::remote::tests::value;

    pub struct Calculator;

    #[async_trait]
    impl TestTunnelService for Calculator {
        async fn calculate(
            &self,
            request: tonic::Request<Expression>,
        ) -> Result<tonic::Response<Value>, Status> {
            let expression = request.get_ref();
            let result = calculate_impl(expression);
            info!("Calculate {expression:?} = {result:?}");
            result.map(tonic::Response::new)
        }
    }

    fn calculate_impl(request: &Expression) -> Result<Value, Status> {
        let result = match request
            .kind
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("null"))?
        {
            expression::Kind::Operation(operation) => {
                let operands = [&operation.left, &operation.right]
                    .map(Option::as_ref)
                    .map(|e| e.ok_or_else(|| Status::invalid_argument("null operand")))
                    .map(|e| e.map(|e| calculate_impl(&*e)));
                let [a, b] = operands;
                let operands = [a??, b??].map(|e| {
                    e.kind
                        .ok_or_else(|| Status::invalid_argument("null result"))
                });
                let [a, b] = operands;
                let operands = (a?, b?);
                match operation.operator() {
                    Operator::UndefinedOperand => {
                        return Err(Status::invalid_argument("null operator"));
                    }
                    Operator::Plus => match operands {
                        (value::Kind::I(a), value::Kind::I(b)) => (a + b).into(),
                        (value::Kind::I(a), value::Kind::F(b)) => (a as f64 + b).into(),
                        (value::Kind::F(a), value::Kind::I(b)) => (a + b as f64).into(),
                        (value::Kind::F(a), value::Kind::F(b)) => (a + b).into(),
                    },
                    Operator::Minus => match operands {
                        (value::Kind::I(a), value::Kind::I(b)) => (a - b).into(),
                        (value::Kind::I(a), value::Kind::F(b)) => (a as f64 - b).into(),
                        (value::Kind::F(a), value::Kind::I(b)) => (a - b as f64).into(),
                        (value::Kind::F(a), value::Kind::F(b)) => (a - b).into(),
                    },
                    Operator::Multiply => match operands {
                        (value::Kind::I(a), value::Kind::I(b)) => (a * b).into(),
                        (value::Kind::I(a), value::Kind::F(b)) => (a as f64 * b).into(),
                        (value::Kind::F(a), value::Kind::I(b)) => (a * b as f64).into(),
                        (value::Kind::F(a), value::Kind::F(b)) => (a * b).into(),
                    },
                    Operator::Divide => match operands {
                        (value::Kind::I(a), value::Kind::I(b)) => (a / b).into(),
                        (value::Kind::I(a), value::Kind::F(b)) => (a as f64 / b).into(),
                        (value::Kind::F(a), value::Kind::I(b)) => (a / b as f64).into(),
                        (value::Kind::F(a), value::Kind::F(b)) => (a / b).into(),
                    },
                }
            }
            expression::Kind::Value(value) => value.clone(),
        };
        Ok(result)
    }
}
