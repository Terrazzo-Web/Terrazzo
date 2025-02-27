use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::pin::Pin;
use std::sync::Arc;

use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use futures::FutureExt;
use futures::future::Shared;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use tokio::sync::oneshot;
use tokio_rustls::TlsConnector;
use tracing::Instrument as _;
use tracing::Span;
use tracing::debug;
use tracing::warn;
use trz_gateway_common::certificate_info::X509CertificateInfo;
use trz_gateway_common::handle::ServerHandle;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::security_configuration::certificate::tls_server::ToTlsServer;
use trz_gateway_common::security_configuration::certificate::tls_server::ToTlsServerError;
use trz_gateway_common::security_configuration::custom_server_certificate_verifier::SignedExtensionCertificateVerifier;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;
use trz_gateway_common::security_configuration::trusted_store::cache::CachedTrustedStoreConfig;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ToTlsClient;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ToTlsClientError;
use trz_gateway_common::tracing::EnableTracingError;

use self::gateway_config::AppConfig;
use self::gateway_config::GatewayConfig;
use self::issuer_config::IssuerConfig;
use self::issuer_config::IssuerConfigError;
use crate::connection::Connections;

mod app;
mod certificate;
pub mod gateway_config;
mod issuer_config;
pub mod root_ca_configuration;
mod tunnel;

#[cfg(test)]
mod tests;

pub struct Server {
    shutdown: Shared<Pin<Box<dyn Future<Output = ()> + Send + Sync>>>,
    root_ca: Arc<X509CertificateInfo>,
    tls_server: RustlsConfig,
    tls_client: TlsConnector,
    connections: Arc<Connections>,
    issuer_config: IssuerConfig,
    app_config: Box<dyn AppConfig>,
}

impl Server {
    pub async fn run<C: GatewayConfig>(
        config: C,
    ) -> Result<(Arc<Self>, ServerHandle<()>), GatewayError<C>> {
        if config.enable_tracing() {
            trz_gateway_common::tracing::enable_tracing()?;
        }

        let (shutdown_rx, terminated_tx, handle) = ServerHandle::new();
        let shutdown_rx: Pin<Box<dyn Future<Output = ()> + Send + Sync>> = Box::pin(shutdown_rx);

        let root_ca = config
            .root_ca()
            .certificate()
            .map_err(|error| GatewayError::RootCa(error.into()))?;
        debug!("Got Root CA: {}", root_ca.display());

        let client_certificate_issuer = config.client_certificate_issuer();
        let issuer_config = IssuerConfig::new(&client_certificate_issuer)?;

        let tls_server = config.tls().to_tls_server().await?;
        debug!("Got TLS server config");

        let tls_client = config
            .root_ca()
            .to_tls_client(SignedExtensionCertificateVerifier {
                store: CachedTrustedStoreConfig::new(client_certificate_issuer)
                    .map_err(GatewayError::CachedTrustedStoreConfig)?,
                signer_name: issuer_config.signer_name.clone(),
            })
            .await?;
        debug!("Got TLS client config");

        let server = Arc::new(Self {
            shutdown: shutdown_rx.shared(),
            root_ca,
            tls_server: RustlsConfig::from_config(Arc::from(tls_server)),
            tls_client: TlsConnector::from(Arc::new(tls_client)),
            connections: Arc::new(Connections::default()),
            issuer_config,
            app_config: Box::new(config.app_config()),
        });

        let (host, port) = (config.host(), config.port());
        let socket_addrs = (host, port).to_socket_addrs();
        let socket_addrs = socket_addrs.map_err(|error| GatewayError::ToSocketAddrs {
            host: host.to_owned(),
            port,
            error,
        })?;

        let mut terminated = vec![];

        for socket_addr in socket_addrs {
            debug!("Setup server on {socket_addr}");
            let task = server.clone().run_endpoint(socket_addr, Span::current());
            let (terminated_tx, terminated_rx) = oneshot::channel();
            terminated.push(terminated_rx);
            tokio::spawn(async move {
                match task.await {
                    Ok(()) => (),
                    Err(error) => warn!("Failed {error}"),
                }
                let _: Result<(), ()> = terminated_tx.send(());
            });
        }

        {
            use futures::future::join_all;
            let all_terminated = join_all(terminated);
            tokio::spawn(
                async move {
                    let _: Vec<Result<(), oneshot::error::RecvError>> = all_terminated.await;
                    let _: Result<(), ()> = terminated_tx.send(());
                }
                .in_current_span(),
            );
        }
        Ok((server, handle))
    }

    async fn run_endpoint(
        self: Arc<Self>,
        socket_addr: SocketAddr,
        span: Span,
    ) -> Result<(), RunGatewayError> {
        let app = self.make_app(span);

        let handle = Handle::new();
        let axum_server =
            axum_server::bind_rustls(socket_addr, self.tls_server.clone()).handle(handle.clone());

        let shutdown = self.shutdown.clone();
        tokio::spawn(
            async move {
                let () = shutdown.await;
                handle.shutdown();
            }
            .in_current_span(),
        );

        debug!("Serving...");
        let () = axum_server
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .map_err(RunGatewayError::Serve)?;
        debug!("Serving: done");
        Ok(())
    }

    pub fn connections(&self) -> &Connections {
        &self.connections
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum GatewayError<C: GatewayConfig> {
    #[error("[{n}] {0}", n = self.name())]
    EnableTracing(#[from] EnableTracingError),

    #[error("[{n}] Failed to get Root CA: {0}", n = self.name())]
    RootCa(Box<dyn std::error::Error>),

    #[error("[{n}] Failed to get the client certificate issuer configuration: {0}", n = self.name())]
    IssuerConfig(#[from] IssuerConfigError<C::ClientCertificateIssuerConfig>),

    #[error("[{n}] Failed to get socket address for {host}:{port}: {error}", n = self.name())]
    ToSocketAddrs {
        host: String,
        port: u16,
        error: std::io::Error,
    },

    #[error("[{n}] {0}", n = self.name())]
    ToTlsServerConfig(#[from] ToTlsServerError<<C::TlsConfig as CertificateConfig>::Error>),

    #[error("[{n}] {0}", n = self.name())]
    ToTlsClientConfig(#[from] ToTlsClientError<<C::RootCaConfig as TrustedStoreConfig>::Error>),

    #[error("[{n}] {0}", n = self.name())]
    CachedTrustedStoreConfig(<C::ClientCertificateIssuerConfig as TrustedStoreConfig>::Error),

    #[error("[{n}] {0}", n = self.name())]
    RunGatewayError(#[from] RunGatewayError),
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum RunGatewayError {
    #[error("[{n}] {0}", n = self.name())]
    Serve(std::io::Error),
}
