pub(super) use graph::Run;

#[terrazzo::graph]
mod graph {
    use std::future::ready;
    use std::sync::Arc;

    use futures::FutureExt as _;
    use tokio::signal::unix::SignalKind;
    use tokio::signal::unix::signal;
    use tracing::debug;
    use trz_gateway_common::dynamic_config::DynamicConfig;
    use trz_gateway_common::dynamic_config::has_diff::DiffArc;
    use trz_gateway_common::dynamic_config::mode::RO;
    use trz_gateway_common::handle::ServerHandle;
    use trz_gateway_common::security_configuration::SecurityConfig;
    use trz_gateway_common::security_configuration::either::EitherConfig;
    use trz_gateway_common::security_configuration::trusted_store::native::NativeTrustedStoreConfig;
    use trz_gateway_server::server::Server as GatewayServer;
    use trz_gateway_server::server::acme::active_challenges::ActiveChallenges;
    use trz_gateway_server::server::acme::certificate_config::AcmeCertificateConfig;

    use crate::assets;
    use crate::backend::RunClientError;
    use crate::backend::RunServerError;
    use crate::backend::Server;
    use crate::backend::auth::AuthConfig;
    use crate::backend::cli::Cli;
    use crate::backend::config::DynConfig;
    use crate::backend::root_ca_config::PrivateRootCa;
    use crate::backend::root_ca_config::PrivateRootCaError;
    use crate::backend::run_client_async;
    use crate::backend::server_config::TerminalBackendServer;
    use crate::backend::server_config::TlsConfig;
    use crate::backend::tls_config::make_tls_config;

    struct RunningServer {
        server: Arc<Server>,
        handle: ServerHandle<()>,
        crash: futures::future::Shared<futures::future::BoxFuture<'static, String>>,
    }

    /// Waits for a server crash or SIGTERM, then stops the server before the client.
    pub async fn run(
        client_handle: Option<ServerHandle<()>>,
        running_server: RunningServer,
    ) -> Result<(), RunServerError> {
        let RunningServer {
            server,
            handle: server_handle,
            crash,
        } = running_server;
        let mut terminate = signal(SignalKind::terminate()).map_err(RunServerError::Signal)?;
        tokio::select! {
            biased;
            crash = crash.clone() => {
                server_handle.stop(crash).await?;
            }
            _ = terminate.recv() => {
                server_handle.stop("Quit").await?;
            }
        }
        drop(server);

        if let Some(client_handle) = client_handle {
            client_handle.stop("Quit").await?;
        }

        Ok(())
    }

    /// Starts the mesh client, aborting initialization if the server crashes.
    /// Returns no handle when the client is disabled.
    async fn client_handle(
        cli: Arc<Cli>,
        config: DiffArc<DynConfig>,
        running_server: &RunningServer,
    ) -> Result<Option<ServerHandle<()>>, RunClientError> {
        let server = &running_server.server;
        let crash = &running_server.crash;
        let client_handle = async {
            match run_client_async(cli, config, server.clone()).await {
                Ok(client_handle) => Ok(Some(client_handle)),
                Err(RunClientError::ClientNotEnabled) => Ok(None),
                Err(error) => Err(error),
            }
        };
        tokio::select! {
            h = client_handle => h,
            crash = crash.clone() => Err(RunClientError::Aborted(crash)),
        }
    }

    /// Installs assets, starts the gateway, and registers enabled remote function services.
    /// Shares the crash notification between client initialization and shutdown.
    async fn running_server(
        backend_config: TerminalBackendServer,
        config: DiffArc<DynConfig>,
    ) -> Result<RunningServer, RunServerError> {
        assets::install::install_assets();
        let (server, server_handle, crash) = GatewayServer::run(backend_config).await?;
        let server = Arc::new(Server::new(server, config.clone()));

        #[cfg(feature = "remote-fn")]
        {
            use crate::backend::client_service::remote_fn_service;

            remote_fn_service::set_remote_fn_server(Arc::downgrade(&server));

            #[cfg(feature = "remote-fn-unary")]
            remote_fn_service::unary::setup();

            #[cfg(feature = "remote-fn-streaming")]
            remote_fn_service::streaming::setup();
        }

        let crash = crash
            .then(|crash| {
                let crash = crash
                    .map(|crash| format!("Crashed: {crash}"))
                    .unwrap_or_else(|_| "Server task dropped".to_owned());
                ready(crash)
            })
            .boxed()
            .shared();

        Ok(RunningServer {
            server,
            handle: server_handle,
            crash,
        })
    }

    /// Assembles the gateway configuration with dynamically refreshed authentication.
    fn backend_config(
        config: DiffArc<DynConfig>,
        root_ca: PrivateRootCa,
        tls_config: Arc<DynamicConfig<TlsConfig, RO>>,
        active_challenges: ActiveChallenges,
    ) -> TerminalBackendServer {
        let auth_config = config
            .server
            .view(|server| DiffArc::from(AuthConfig::new(server)))
            .into();
        TerminalBackendServer {
            config,
            root_ca,
            tls_config,
            auth_config,
            active_challenges,
        }
    }

    /// Builds a dynamic TLS configuration from ACME settings and the renewal threshold.
    /// Uses the private root CA when Let's Encrypt is disabled.
    fn tls_config(
        config: &DiffArc<DynConfig>,
        root_ca: PrivateRootCa,
        active_challenges: ActiveChallenges,
    ) -> Arc<DynamicConfig<TlsConfig, RO>> {
        let dynamic_acme_config = config.letsencrypt.clone();
        let joined_config = config.letsencrypt.zip(
            &config
                .server
                .view_diff(|server_config| server_config.certificate_renewal_threshold),
        );
        joined_config.view(move |(letsencrypt, certificate_renewal_threshold)| {
            debug!("Refresh TLS config");
            if let Some(letsencrypt) = &**letsencrypt {
                EitherConfig::Right(SecurityConfig {
                    trusted_store: NativeTrustedStoreConfig,
                    certificate: AcmeCertificateConfig::new(
                        dynamic_acme_config.clone(),
                        letsencrypt.clone(),
                        active_challenges.clone(),
                        *certificate_renewal_threshold,
                    ),
                })
            } else {
                EitherConfig::Left(make_tls_config(&root_ca).unwrap())
            }
        })
    }

    /// Loads or creates the private root CA used to issue client certificates.
    fn root_ca(config: &DiffArc<DynConfig>) -> Result<PrivateRootCa, PrivateRootCaError> {
        PrivateRootCa::load(config)
    }

    /// Creates the shared registry of active ACME certificate challenges.
    fn active_challenges() -> ActiveChallenges {
        ActiveChallenges::default()
    }
}
