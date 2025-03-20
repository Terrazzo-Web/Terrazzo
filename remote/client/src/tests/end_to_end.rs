use std::sync::Arc;
use std::time::Duration;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use tokio::time::error::Elapsed;
use tracing::Instrument as _;
use tracing::info;
use tracing::info_span;
use trz_gateway_common::certificate_info::CertificateInfo;
use trz_gateway_common::handle::ServerHandle;
use trz_gateway_common::handle::ServerStopError;
use trz_gateway_common::id::ClientName;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificate;
use trz_gateway_server::auth_code::AuthCode;
use trz_gateway_server::server::GatewayError;
use trz_gateway_server::server::Server;

use super::test_client_config::TestClientConfig;
use super::test_gateway_config::TestGatewayConfig;
use super::test_gateway_config::use_temp_dir;
use super::test_tunnel_config::TestTunnelConfig;
use crate::client::Client;
use crate::client::NewClientError;
use crate::client::connect::ConnectError;
use crate::load_client_certificate::LoadClientCertificateError;
use crate::load_client_certificate::load_client_certificate;

const CLIENT_CERT_FILENAME: CertificateInfo<&str> = CertificateInfo {
    certificate: "client-cert.pem",
    private_key: "client-key.pem",
};

pub struct EndToEnd<'t> {
    pub client: Client,
    #[expect(unused)]
    pub client_certificate: Arc<PemCertificate>,
    pub server: Arc<Server>,
    pub client_handle: Box<dyn FnOnce() -> ServerHandle<()> + 't>,
}

impl<'t> EndToEnd<'t> {
    pub async fn run(
        test: impl AsyncFnOnce(EndToEnd) -> Result<(), Box<dyn std::error::Error>> + Send,
    ) -> Result<(), EndToEndError> {
        let temp_dir = use_temp_dir();

        let gateway_config = TestGatewayConfig::new();
        let (server, server_handle) = Server::run(gateway_config.clone())
            .instrument(info_span!("Server"))
            .await
            .map_err(EndToEndError::SetupServer)?;
        info!("Started the server");

        let client_name = ClientName::from("EndToEndClient");
        let client_config = TestClientConfig::new(gateway_config.clone(), client_name.clone());

        let auth_code = AuthCode::current().to_string();
        let client_certificate = Arc::new(
            load_client_certificate(
                &client_config,
                auth_code.into(),
                CLIENT_CERT_FILENAME.map(|filename| temp_dir.path().join(filename)),
            )
            .await
            .map_err(EndToEndError::LoadClientCertificate)?,
        );
        info!("Got the client certificate");

        let tunnel_config = TestTunnelConfig::new(client_config, client_certificate.clone());
        let client = Client::new(tunnel_config).map_err(EndToEndError::NewClient)?;
        let client_handle = client
            .run()
            .instrument(info_span!("Client"))
            .await
            .map_err(EndToEndError::RunClientError)?;
        info!("The client is running");

        let mut client_handle = Some(client_handle);

        let test = test(EndToEnd {
            client,
            server,
            client_certificate,
            client_handle: Box::new(|| client_handle.take().unwrap()),
        });
        let test = tokio::time::timeout(Duration::from_secs(60), test);
        let () = test
            .await
            .map_err(EndToEndError::TestTimeout)?
            .map_err(EndToEndError::TestFailure)?;

        let () = server_handle
            .stop("Stopping server")
            .await
            .map_err(EndToEndError::StopServer)?;
        info!("Server stopped");
        if let Some(client_handle) = client_handle.take() {
            let () = client_handle
                .stop("Stopping client")
                .await
                .map_err(EndToEndError::StopClient)?;
            info!("Client stopped");
        }
        drop(temp_dir);
        Ok(())
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum EndToEndError {
    #[error("[{n}] {0}", n = self.name())]
    SetupServer(GatewayError<Arc<TestGatewayConfig>>),

    #[error("[{n}] {0}", n = self.name())]
    LoadClientCertificate(LoadClientCertificateError<TestClientConfig<Arc<TestGatewayConfig>>>),

    #[error("[{n}] {0}", n = self.name())]
    NewClient(NewClientError<TestTunnelConfig<Arc<TestGatewayConfig>>>),

    #[error("[{n}] {0}", n = self.name())]
    RunClientError(ConnectError),

    #[error("[{n}] {0}", n = self.name())]
    TestTimeout(Elapsed),

    #[error("[{n}] {0}", n = self.name())]
    TestFailure(Box<dyn std::error::Error>),

    #[error("[{n}] {0}", n = self.name())]
    StopServer(ServerStopError),

    #[error("[{n}] {0}", n = self.name())]
    StopClient(ServerStopError),
}
