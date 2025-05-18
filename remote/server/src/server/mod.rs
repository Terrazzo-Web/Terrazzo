use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;

use axum_server::Handle;
use axum_server::accept::DefaultAcceptor;
use axum_server::tls_rustls::RustlsAcceptor;
use axum_server::tls_rustls::RustlsConfig;
use futures::FutureExt;
use futures::future::Shared;
use gateway_config::app_config::AppConfig;
use http_or_https::HttpOrHttps;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use tokio::sync::oneshot;
use tokio_rustls::TlsConnector;
use tracing::Instrument as _;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::info_span;
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

use self::gateway_config::GatewayConfig;
use self::issuer_config::IssuerConfig;
use self::issuer_config::IssuerConfigError;
use crate::connection::Connections;

pub mod acme;
mod app;
mod certificate;
pub mod gateway_config;
mod http_or_https;
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
    ) -> Result<
        (
            Arc<Self>,
            ServerHandle<()>,
            oneshot::Receiver<RunGatewayError>,
        ),
        GatewayError<C>,
    > {
        if config.enable_tracing() {
            trz_gateway_common::tracing::enable_tracing()?;
        }

        let _span = info_span!("Server").entered();

        let (shutdown_rx, terminated_tx, handle) = ServerHandle::new("Server");
        let shutdown_rx: Pin<Box<dyn Future<Output = ()> + Send + Sync>> = Box::pin(shutdown_rx);

        let root_ca_config = config.root_ca();
        let root_ca = root_ca_config
            .certificate()
            .map_err(|error| GatewayError::RootCa(error.into()))?;
        info!("Root CA: {:?}", root_ca.certificate.subject_name());
        debug!("Root CA details: {}", root_ca.display());

        let client_certificate_issuer = config.client_certificate_issuer();
        let issuer_config = IssuerConfig::new(&client_certificate_issuer)?;
        info!("Signer certificate: {:?}", issuer_config.signer_name);
        debug!(
            "Signer certificate details: {}",
            issuer_config.signer.display()
        );

        let tls_server = config.tls().to_tls_server()?;
        debug!("Got TLS server config");

        let tls_client = root_ca_config.to_tls_client(SignedExtensionCertificateVerifier {
            store: CachedTrustedStoreConfig::new(client_certificate_issuer)
                .map_err(GatewayError::CachedTrustedStoreConfig)?,
            signer_name: issuer_config.signer_name.clone(),
        })?;
        debug!("Got TLS client config");

        let server = Arc::new(Self {
            shutdown: shutdown_rx.shared(),
            root_ca,
            tls_server: RustlsConfig::from_config(tls_server),
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
        drop(config);

        let mut terminated = vec![];

        let (server_crash_tx, server_crash_rx) = oneshot::channel();
        let server_crash_tx = Arc::new(Mutex::new(Some(server_crash_tx)));
        for socket_addr in socket_addrs {
            let _span = info_span!("Listen", %socket_addr).entered();
            info!("Setup server");
            let task = server.clone().run_endpoint(socket_addr);
            let (terminated_tx, terminated_rx) = oneshot::channel();
            terminated.push(terminated_rx);
            let server_crash_tx = server_crash_tx.clone();
            tokio::spawn(
                async move {
                    match task.await {
                        Ok(()) => (),
                        Err(error) => {
                            error!("Failed {error}");
                            if let Some(server_crash_tx) =
                                server_crash_tx.lock().expect("server_crash_tx").take()
                            {
                                let _ = server_crash_tx.send(error);
                            }
                        }
                    }
                    let _: Result<(), ()> = terminated_tx.send(());
                }
                .in_current_span(),
            );
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
        Ok((server, handle, server_crash_rx))
    }

    async fn run_endpoint(self: Arc<Self>, socket_addr: SocketAddr) -> Result<(), RunGatewayError> {
        let app = self.make_app();

        let handle = Handle::new();
        let axum_server = axum_server::bind(socket_addr)
            .acceptor(HttpOrHttps {
                tls: RustlsAcceptor::new(self.tls_server.clone()),
                plaintext: DefaultAcceptor,
            })
            .handle(handle.clone());

        let shutdown = self.shutdown.clone();
        tokio::spawn(
            async move {
                let () = shutdown.await;
                handle.shutdown();
            }
            .in_current_span(),
        );

        info!("Serving...");
        let () = axum_server
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .map_err(RunGatewayError::Serve)?;
        info!("Serving: done");
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
