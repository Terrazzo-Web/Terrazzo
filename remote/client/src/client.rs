use std::sync::Arc;

use connect::ConnectError;
use connect::TunnelError;
use nameth::nameth;
use nameth::NamedEnumValues as _;
use trz_gateway_common::declare_identifier;
use trz_gateway_common::handle::ServerHandle;
use trz_gateway_common::security_configuration::certificate::tls_server::ToTlsServer as _;
use trz_gateway_common::security_configuration::certificate::tls_server::ToTlsServerError;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ChainOnlyServerCertificateVerifier;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ToTlsClient as _;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ToTlsClientError;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;
use trz_gateway_common::tracing::EnableTracingError;

use crate::client_configuration::ClientConfig;

pub mod certificate;
pub mod connect;
mod health;

pub struct Client {
    uri: String,
    tls_client: tokio_tungstenite::Connector,
    tls_server: tokio_rustls::TlsAcceptor,
}

declare_identifier!(AuthCode);

impl Client {
    pub async fn new<C: ClientConfig>(config: C) -> Result<Self, NewClientError<C>> {
        let tls_client = config
            .server_pki()
            .to_tls_client(ChainOnlyServerCertificateVerifier)
            .await?;
        let tls_server = config.tls().to_tls_server().await?;

        Ok(Client {
            uri: format!("{}/remote/tunnel/{}", config.base_url(), config.client_id()),
            tls_client: tokio_tungstenite::Connector::Rustls(tls_client.into()),
            tls_server: tokio_rustls::TlsAcceptor::from(Arc::new(tls_server)),
        })
    }

    pub async fn run(&self) -> Result<ServerHandle<Result<(), TunnelError>>, RunClientError> {
        let (shutdown_rx, terminated_tx, handle) = ServerHandle::new();
        let () = self.connect(shutdown_rx, terminated_tx).await?;
        Ok(handle)
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum NewClientError<C: ClientConfig> {
    #[error("[{n}] {0}", n = self.name())]
    EnableTracing(#[from] EnableTracingError),

    #[error("[{n}] {0}", n = self.name())]
    ToTlsClient(#[from] ToTlsClientError<<C::ServerPkiConfig as TrustedStoreConfig>::Error>),

    #[error("[{n}] {0}", n = self.name())]
    ToTlsServer(#[from] ToTlsServerError<<C::TlsConfig as CertificateConfig>::Error>),
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum RunClientError {
    #[error("[{n}] {0}", n = self.name())]
    Connect(#[from] ConnectError),
}
