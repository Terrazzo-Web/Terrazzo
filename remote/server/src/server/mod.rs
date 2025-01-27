use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;

use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use futures::future::Shared;
use futures::FutureExt;
use nameth::nameth;
use nameth::NamedEnumValues as _;
use tokio::sync::oneshot;
use tracing::info;
use tracing::info_span;
use tracing::trace;
use tracing::warn;
use tracing::Instrument as _;
use trz_gateway_common::tracing::EnableTracingError;

use self::gateway_configuration::GatewayConfig;
use crate::security_configuration::certificate::Certificate;
use crate::security_configuration::certificate::CertificateConfig;
use crate::security_configuration::certificate::ToRustlsConfigError;

mod app;
mod certificate;
pub mod gateway_configuration;
pub mod root_ca_configuration;
mod tunnel;

#[cfg(test)]
mod tests;

pub struct Server<C> {
    config: C,
    shutdown: Shared<oneshot::Receiver<String>>,
    root_ca: Certificate,
    tls_config: RustlsConfig,
}

impl<C: GatewayConfig> Server<C> {
    pub async fn run(
        config: C,
    ) -> Result<(oneshot::Sender<String>, oneshot::Receiver<()>), GatewayError<C>> {
        if config.enable_tracing() {
            trz_gateway_common::tracing::enable_tracing()?;
        }

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<String>();
        let shutdown_rx = shutdown_rx.shared();

        let root_ca = config
            .root_ca()
            .certificate()
            .map_err(|error| GatewayError::RootCa(error.into()))?;
        trace!("Got Root CA");

        let tls_config = config.tls().to_rustls_config().await?;
        trace!("Got TLS config");

        let server = Arc::new(Self {
            config,
            shutdown: shutdown_rx,
            root_ca,
            tls_config,
        });

        let (host, port) = server.socket_addr();
        let socket_addrs = (host, port).to_socket_addrs();
        let socket_addrs = socket_addrs.map_err(|error| GatewayError::ToSocketAddrs {
            host: host.to_owned(),
            port,
            error,
        })?;

        let mut terminated = vec![];

        for socket_addr in socket_addrs {
            let task = server.clone().run_endpoint(socket_addr);
            let (terminated_tx, terminated_rx) = oneshot::channel();
            terminated.push(terminated_rx);
            tokio::spawn(
                async move {
                    match task.await {
                        Ok(()) => (),
                        Err(error) => warn!("Failed {error}"),
                    }
                    let _: Result<(), ()> = terminated_tx.send(());
                }
                .instrument(info_span!("Serving", %socket_addr)),
            );
        }

        let all_terminated = {
            use futures::future::join_all;
            let all_terminated = join_all(terminated);
            let (all_terminated_tx, all_terminated_rx) = oneshot::channel();
            tokio::spawn(
                async move {
                    let _: Vec<Result<(), oneshot::error::RecvError>> = all_terminated.await;
                    let _: Result<(), ()> = all_terminated_tx.send(());
                }
                .in_current_span(),
            );
            all_terminated_rx
        };
        Ok((shutdown_tx, all_terminated))
    }

    async fn run_endpoint(self: Arc<Self>, socket_addr: SocketAddr) -> Result<(), GatewayError<C>> {
        let app = self.make_app();

        let handle = Handle::new();
        let axum_server =
            axum_server::bind_rustls(socket_addr, self.tls_config.clone()).handle(handle.clone());

        let shutdown = self.shutdown.clone();
        tokio::spawn(
            async move {
                match shutdown.await {
                    Ok(message) => info!("Server shutdown: {message}"),
                    Err(oneshot::error::RecvError { .. }) => warn!("Server handle dropped!"),
                }
                handle.graceful_shutdown(Some(Duration::from_secs(30)));
            }
            .in_current_span(),
        );

        trace!("Serving...");
        let () = axum_server
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .map_err(GatewayError::Serve)?;
        trace!("Serving: done");
        Ok(())
    }

    fn socket_addr(&self) -> (&str, u16) {
        (self.config.host(), self.config.port())
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum GatewayError<C: GatewayConfig> {
    #[error("[{n}] {0}", n = self.name())]
    EnableTracing(#[from] EnableTracingError),

    #[error("[{n}] Failed to get Root CA: {0}", n = self.name())]
    RootCa(Box<dyn std::error::Error>),

    #[error("[{n}] Failed to get socket address for {host}:{port}: {error}", n = self.name())]
    ToSocketAddrs {
        host: String,
        port: u16,
        error: std::io::Error,
    },

    #[error("[{n}] {0}", n = self.name())]
    ToRustlsConfig(#[from] ToRustlsConfigError<<C::TlsConfig as CertificateConfig>::Error>),

    #[error("[{n}] {0}", n = self.name())]
    Serve(std::io::Error),
}
