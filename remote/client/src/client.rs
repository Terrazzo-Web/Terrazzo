use std::sync::Arc;
use std::time::Instant;

use connect::ConnectError;
use futures::FutureExt;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use tokio::sync::oneshot;
use tracing::Instrument;
use tracing::error;
use tracing::info;
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
    pub fn new<C: TunnelConfig>(config: C) -> Result<Arc<Self>, NewClientError<C>> {
        let tls_client = config
            .gateway_pki()
            .to_tls_client(ChainOnlyServerCertificateVerifier)?;
        let tls_server = config.client_certificate().to_tls_server()?;
        Ok(Arc::new(Client {
            client_name: config.client_name(),
            uri: format!(
                "{}/remote/tunnel/{}",
                config.base_url(),
                config.client_name()
            ),
            tls_client: tokio_tungstenite::Connector::Rustls(tls_client.into()),
            tls_server: tokio_rustls::TlsAcceptor::from(tls_server),
            client_service: Arc::new(config.client_service()),
            retry_strategy: config.retry_strategy(),
        }))
    }

    /// Runs the client and returns a handle to stop the client.
    pub async fn run(self: &Arc<Self>) -> Result<ServerHandle<()>, ConnectError> {
        let this = self.clone();
        let client_name = &this.client_name;
        let span = info_span!("Run", %client_name);
        async move {
            let client_id = ClientId::from(Uuid::new_v4().to_string());
            info!(%client_id, "Allocated new client id");
            let retry_strategy0 = this.retry_strategy.clone();
            let mut retry_strategy = retry_strategy0.clone();
            let (shutdown_rx, terminated_tx, handle) = ServerHandle::new("Client");
            let shutdown_rx = shutdown_rx.shared();
            let (serving_tx, serving_rx) = oneshot::channel();

            let task = async move {
                let mut serving_tx = Some(serving_tx);
                loop {
                    let start = Instant::now();
                    let result = this
                        .connect(client_id.clone(), shutdown_rx.clone(), &mut serving_tx)
                        .await;
                    if let Some(()) = shutdown_rx.peek() {
                        break;
                    }
                    let uptime = Instant::now() - start;
                    match result {
                        Ok(()) => info!(
                            "Connection closed, retrying in {}...",
                            humantime::format_duration(retry_strategy.delay)
                        ),
                        Err(error) => warn!(
                            %error,
                            "Connection failed, retrying in {}...",
                            humantime::format_duration(retry_strategy.delay)
                        ),
                    }
                    if uptime < retry_strategy0.max_delay {
                        retry_strategy.wait().await;
                    } else {
                        retry_strategy = retry_strategy0.clone();
                    }
                }
                let _ = terminated_tx.send(());
            };
            tokio::spawn(task.in_current_span());
            let _ = serving_rx.await;
            Ok(handle)
        }
        .instrument(span)
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
