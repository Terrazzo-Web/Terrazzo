use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tracing::info;
use trz_gateway_common::certificate_info::CertificateInfo;
use trz_gateway_common::handle::ServerHandle;
use trz_gateway_common::id::ClientId;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificate;
use trz_gateway_server::auth_code::AuthCode;
use trz_gateway_server::server::Server;

use super::test_client_config::TestClientConfig;
use super::test_gateway_config::TestGatewayConfig;
use super::test_gateway_config::use_temp_dir;
use super::test_tunnel_config::TestTunnelConfig;
use crate::client::Client;
use crate::client::connect::TunnelError;
use crate::load_client_certificate::load_client_certificate;

const CLIENT_CERT_FILENAME: CertificateInfo<&str> = CertificateInfo {
    certificate: "client-cert.pem",
    private_key: "client-key.pem",
};

pub struct EndToEnd<'t> {
    pub client_id: ClientId,
    #[expect(unused)]
    pub client_certificate: Arc<PemCertificate>,
    pub server: Arc<Server>,
    #[expect(unused)]
    pub client_handle: Box<dyn FnOnce() -> ServerHandle<Result<(), TunnelError>> + 't>,
}

impl<'t> EndToEnd<'t> {
    pub async fn run<F: Future<Output = Result<(), Box<dyn std::error::Error>>>>(
        test: impl FnOnce(EndToEnd) -> F + Send,
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
        let client_certificate = Arc::new(
            load_client_certificate(
                client_config.clone(),
                auth_code.into(),
                CLIENT_CERT_FILENAME.map(|filename| temp_dir.path().join(filename)),
            )
            .await?,
        );
        info!("Got the client certificate");

        let tunnel_config =
            TestTunnelConfig::new(client_config.clone(), client_certificate.clone());
        let client = Client::new(tunnel_config).await?;
        let client_handle = client.run().await?;
        info!("The client is running");

        let mut client_handle = Some(client_handle);

        let test = test(EndToEnd {
            client_id,
            server,
            client_certificate,
            client_handle: Box::new(|| client_handle.take().unwrap()),
        });
        let test = tokio::time::timeout(Duration::from_secs(5), test);
        let () = test.await??;

        let () = server_handle.stop("End of test").await?;
        if let Some(client_handle) = client_handle.take() {
            let client_disconnected = client_handle.stopped().await?.unwrap_err();
            assert!(matches!(client_disconnected, TunnelError::Disconnected));
        }
        Ok(())
    }
}
