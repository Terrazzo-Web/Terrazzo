#![cfg(test)]

use std::sync::Arc;

use test_client_config::TestClientConfig;
use test_gateway_config::TestGatewayConfig;
use test_tunnel_config::TestTunnelConfig;
use tracing::info;
use trz_gateway_common::certificate_info::CertificateInfo;
use trz_gateway_server::server::Server;

use self::test_gateway_config::use_temp_dir;
use crate::client::Client;
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
    let temp_dir = use_temp_dir();

    let gateway_config = TestGatewayConfig::new();
    let server_handle = Server::run(gateway_config.clone()).await?;
    info!("Started the server");

    let client_id = "EndToEndClient".into();
    let client_config = Arc::new(TestClientConfig::new(gateway_config.clone(), client_id));

    let auth_code = trz_gateway_server::auth_code::AuthCode::current().to_string();
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

    let () = server_handle.stop("End of test").await?;
    let () = client_handle.stopped().await??;
    Ok(())
}
