#![cfg(test)]

use std::future::Future;
use std::sync::Arc;

use test_client_config::TestClientConfig;
use test_gateway_config::TestGatewayConfig;
use test_tunnel_config::TestTunnelConfig;
use tracing::info;
use trz_gateway_common::certificate_info::CertificateInfo;
use trz_gateway_common::id::ClientId;
use trz_gateway_common::protos::terrazzo::remote::tests::Expression;
use trz_gateway_common::protos::terrazzo::remote::tests::Value;
use trz_gateway_common::protos::terrazzo::remote::tests::test_tunnel_service_client::TestTunnelServiceClient;
use trz_gateway_server::auth_code::AuthCode;
use trz_gateway_server::server::Server;

use self::test_gateway_config::use_temp_dir;
use crate::client::Client;
use crate::client::connect::TunnelError;
use crate::load_client_certificate::load_client_certificate;

mod test_client_config;
mod test_gateway_config;
mod test_tunnel_config;

const CLIENT_CERT_FILENAME: CertificateInfo<&str> = CertificateInfo {
    certificate: "client-cert.pem",
    private_key: "client-key.pem",
};

#[tokio::test]
async fn end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    EndToEnd::run(|EndToEnd { client_id, server }| async move {
        let channel = server
            .connections()
            .get_client(&client_id)
            .ok_or(format!("Client {client_id} not found"))?;
        let mut grpc_client = TestTunnelServiceClient::new(channel);
        let response = grpc_client
            .calculate(tonic::Request::new(
                { Expression::from(5) + Expression::from(2) * 3.into() }.into(),
            ))
            .await?
            .into_inner();
        assert_eq!(Value::from(11), response);
        Ok(())
    })
    .await
}

struct EndToEnd {
    client_id: ClientId,
    server: Arc<Server>,
}

impl EndToEnd {
    async fn run<F: Future<Output = Result<(), Box<dyn std::error::Error>>>>(
        test: impl FnOnce(Self) -> F,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = use_temp_dir();

        let gateway_config = TestGatewayConfig::new();
        let (server, server_handle) = Server::run(gateway_config.clone()).await?;
        info!("Started the server");

        let client_id = ClientId::from("EndToEndClient");
        let client_config = Arc::new(TestClientConfig::new(
            gateway_config.clone(),
            client_id.clone(),
        ));

        let auth_code = AuthCode::current().to_string();
        let client_certificate = load_client_certificate(
            client_config.clone(),
            auth_code.into(),
            CLIENT_CERT_FILENAME.map(|filename| temp_dir.path().join(filename)),
        )
        .await?
        .into();
        info!("Got the client certificate");

        let tunnel_config = TestTunnelConfig::new(client_config.clone(), client_certificate);
        let client = Client::new(tunnel_config).await?;
        let client_handle = client.run().await?;
        info!("The client is running");

        let () = test(Self { client_id, server }).await?;

        let () = server_handle.stop("End of test").await?;
        let client_disconnected = client_handle.stopped().await?.unwrap_err();
        assert!(matches!(client_disconnected, TunnelError::Disconnected));
        Ok(())
    }
}
