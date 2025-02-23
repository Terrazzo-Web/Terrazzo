use std::sync::Arc;

use connect::ConnectError;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use trz_gateway_common::declare_identifier;
use trz_gateway_common::handle::ServerHandle;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::security_configuration::certificate::tls_server::ToTlsServer as _;
use trz_gateway_common::security_configuration::certificate::tls_server::ToTlsServerError;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ChainOnlyServerCertificateVerifier;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ToTlsClient as _;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ToTlsClientError;

use crate::client_service::ClientService;
use crate::tunnel_config::TunnelConfig;

pub mod certificate;
pub mod connect;
mod connection;
mod health;
mod to_async_io;

pub struct Client {
    uri: String,
    tls_client: tokio_tungstenite::Connector,
    tls_server: tokio_rustls::TlsAcceptor,
    client_service: Arc<dyn ClientService>,
}

declare_identifier!(AuthCode);

impl Client {
    /// Creates a new [Client].
    pub async fn new<C: TunnelConfig>(config: C) -> Result<Self, NewClientError<C>> {
        let tls_client = config
            .gateway_pki()
            .to_tls_client(ChainOnlyServerCertificateVerifier)
            .await?;
        let tls_server = config.client_certificate().to_tls_server().await?;
        Ok(Client {
            uri: format!("{}/remote/tunnel/{}", config.base_url(), config.client_id()),
            tls_client: tokio_tungstenite::Connector::Rustls(tls_client.into()),
            tls_server: tokio_rustls::TlsAcceptor::from(Arc::new(tls_server)),
            client_service: Arc::new(config.client_service()),
        })
    }

    pub async fn run(&self) -> Result<ServerHandle<()>, RunClientError> {
        let (shutdown_rx, terminated_tx, handle) = ServerHandle::new();
        let () = self.connect(shutdown_rx, terminated_tx).await?;
        Ok(handle)
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
