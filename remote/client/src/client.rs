use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use connect::ConnectError;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use tracing::Instrument as _;
use tracing::info_span;
use tracing::warn;
use trz_gateway_common::declare_identifier;
use trz_gateway_common::handle::ServerHandle;
use trz_gateway_common::id::ClientId;
use trz_gateway_common::id::ClientName;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::security_configuration::certificate::tls_server::ToTlsServer as _;
use trz_gateway_common::security_configuration::certificate::tls_server::ToTlsServerError;
use trz_gateway_common::security_configuration::custom_server_certificate_verifier::ChainOnlyServerCertificateVerifier;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ToTlsClient as _;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ToTlsClientError;
use uuid::Uuid;

use crate::client_service::ClientService;
use crate::retry_strategy::RetryStrategy;
use crate::tunnel_config::TunnelConfig;

pub mod certificate;
pub mod connect;
mod connection;
mod health;

/// The [Client].
///
/// It creates a WebSocket tunnel with the Terrazzo Gateway, and then runs a
/// gRPC server that listens to requests sent or forwarded by the Terrazzo
/// Gateway over the WebSocket tunnel.
pub struct Client {
    pub client_name: ClientName,
    uri: String,
    tls_client: tokio_tungstenite::Connector,
    tls_server: tokio_rustls::TlsAcceptor,
    client_service: Arc<dyn ClientService>,
    retry_strategy: RetryStrategy,
}

declare_identifier!(AuthCode);

impl Client {
    /// Creates a new [Client].
    pub async fn new<C: TunnelConfig>(config: C) -> Result<Self, NewClientError<C>> {
        let tls_client = config
            .gateway_pki()
            .to_tls_client(ChainOnlyServerCertificateVerifier)?;
        let tls_server = config.client_certificate().to_tls_server().await?;
        Ok(Client {
            client_name: config.client_name(),
            uri: format!(
                "{}/remote/tunnel/{}",
                config.base_url(),
                config.client_name()
            ),
            tls_client: tokio_tungstenite::Connector::Rustls(tls_client.into()),
            tls_server: tokio_rustls::TlsAcceptor::from(Arc::new(tls_server)),
            client_service: Arc::new(config.client_service()),
            retry_strategy: config.retry_strategy(),
        })
    }

    /// Runs the client and returns a handle to stop the client.
    pub async fn run(&self) -> Result<ServerHandle<()>, RunClientError> {
        let client_name = &self.client_name;
        let client_id = ClientId::from(Uuid::new_v4().to_string());
        let mut retry_strategy = self.retry_strategy.clone();
        async {
            loop {
                let (shutdown_rx, terminated_tx, handle) = ServerHandle::new();
                let start = Instant::now();
                let result = self
                    .connect(client_id.clone(), shutdown_rx, terminated_tx)
                    .await;
                let uptime = Instant::now() - start;
                match result {
                    Ok(()) => break Ok(handle),
                    Err(error) if uptime < Duration::from_secs(15) => break Err(error)?,
                    Err(error) => warn!(
                        "Connection failed, retrying in {}... error={error}",
                        humantime::format_duration(retry_strategy.delay)
                    ),
                }
                retry_strategy.wait().await;
            }
        }
        .instrument(info_span!("Run", %client_name, %client_id))
        .await
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum NewClientError<C: TunnelConfig> {
    #[error("[{n}] {0}", n = self.name())]
    ToTlsClient(#[from] ToTlsClientError<<C::GatewayPki as TrustedStoreConfig>::Error>),

    #[error("[{n}] {0}", n = self.name())]
    ToTlsServer(#[from] ToTlsServerError<<C::ClientCertificate as CertificateConfig>::Error>),
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum RunClientError {
    #[error("[{n}] {0}", n = self.name())]
    Connect(#[from] ConnectError),
}
