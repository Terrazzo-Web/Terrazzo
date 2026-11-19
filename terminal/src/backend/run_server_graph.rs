pub(super) use graph::Run;

/// Initializes the server and client, then waits for shutdown.
///
/// ```graph
///    ┌──────────────────────────────┐
///    │                              │
/// root_ca ──→ tls_config ──→ backend_config ──→ running_server ──→ client_handle ──→ run
///                  │                │                  │                              │
///                  │                │                  └──────────────────────────────┘
///                  │                │
///   active_challenges───────────────┘
/// ```
#[terrazzo::graph]
mod graph {
    use std::future::ready;
    use std::sync::Arc;

    use futures::FutureExt as _;
    use tokio::signal::unix::SignalKind;
    use tokio::signal::unix::signal;
    use trz_gateway_common::handle::ServerHandle;
    use trz_gateway_server::server::Server as GatewayServer;
    use trz_gateway_server::server::acme::active_challenges::ActiveChallenges;

    use crate::assets;
    use crate::backend::RunClientError;
    use crate::backend::RunServerError;
    use crate::backend::Server;
    use crate::backend::cli::Cli;
    use crate::backend::config::DynConfig;
    use crate::backend::root_ca_config::PrivateRootCa;
    use crate::backend::root_ca_config::PrivateRootCaError;
    use crate::backend::run_client_async;
    use crate::backend::server_config::TerminalBackendServer;
    use crate::backend::server_config::TlsConfig;

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
        server.config().shutdown();
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
        config: Arc<DynConfig>,
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
        config: Arc<DynConfig>,
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
        config: Arc<DynConfig>,
        root_ca: PrivateRootCa,
        tls_config: tokio::sync::watch::Receiver<TlsConfig>,
        active_challenges: ActiveChallenges,
    ) -> TerminalBackendServer {
        let auth_config = config.server.clone();
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
        config: &Arc<DynConfig>,
        root_ca: PrivateRootCa,
        active_challenges: ActiveChallenges,
    ) -> tokio::sync::watch::Receiver<TlsConfig> {
        let runtime = super::RuntimeTls::new(config, root_ca, active_challenges);
        let receiver = runtime.sender.subscribe();
        assert!(config.tls.set(runtime).is_ok(), "TLS initialized twice");
        receiver
    }

    /// Loads or creates the private root CA used to issue client certificates.
    fn root_ca(config: &Arc<DynConfig>) -> Result<PrivateRootCa, PrivateRootCaError> {
        PrivateRootCa::load(config)
    }

    /// Creates the shared registry of active ACME certificate challenges.
    fn active_challenges() -> ActiveChallenges {
        ActiveChallenges::default()
    }
}

/// Owns one stable TLS watch channel. Input changes are prepared synchronously
/// under the config mutation lock before consumers are notified.
pub(super) struct RuntimeTls {
    sender: tokio::sync::watch::Sender<crate::backend::server_config::TlsConfig>,
    inputs: std::sync::Mutex<(
        Option<Arc<trz_gateway_server::server::acme::AcmeConfig<Arc<std::path::Path>>>>,
        std::time::Duration,
    )>,
    fallback: crate::backend::server_config::TlsConfig,
    active_challenges: trz_gateway_server::server::acme::active_challenges::ActiveChallenges,
    config: std::sync::Weak<crate::backend::config::DynConfig>,
}

use crate::backend::config::WatchConfig as _;
use std::sync::Arc;

impl RuntimeTls {
    fn new(
        config: &Arc<crate::backend::config::DynConfig>,
        root_ca: crate::backend::root_ca_config::PrivateRootCa,
        active_challenges: trz_gateway_server::server::acme::active_challenges::ActiveChallenges,
    ) -> Self {
        let fallback = trz_gateway_common::security_configuration::either::EitherConfig::Left(
            crate::backend::tls_config::make_tls_config(&root_ca)
                .expect("private root CA TLS config"),
        );
        let this = Self {
            sender: tokio::sync::watch::channel(fallback.clone()).0,
            inputs: std::sync::Mutex::new((None, std::time::Duration::ZERO)),
            fallback,
            active_challenges,
            config: Arc::downgrade(config),
        };
        this.refresh(config);
        this
    }

    pub(super) fn refresh(&self, config: &crate::backend::config::DynConfig) {
        use trz_gateway_common::security_configuration::{
            SecurityConfig, either::EitherConfig, trusted_store::native::NativeTrustedStoreConfig,
        };
        use trz_gateway_server::server::acme::{
            DynamicAcmeConfig, certificate_config::AcmeCertificateConfig,
        };
        let acme = config.letsencrypt.get();
        let threshold = config.server.with(|s| s.certificate_renewal_threshold);
        let mut inputs = self.inputs.lock().unwrap();
        let unchanged = match (&inputs.0, &acme) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        if unchanged && inputs.1 == threshold {
            return;
        }
        let tls = if let Some(acme) = &acme {
            let weak = self.config.clone();
            let expected = acme.clone();
            let updates = DynamicAcmeConfig::from_update(move |new| {
                if let Some(config) = weak.upgrade() {
                    config.update_acme(&expected, |acme| *acme = new);
                }
            });
            EitherConfig::Right(SecurityConfig {
                trusted_store: NativeTrustedStoreConfig,
                certificate: AcmeCertificateConfig::new(
                    updates,
                    acme.clone().into(),
                    self.active_challenges.clone(),
                    threshold,
                ),
            })
        } else {
            self.fallback.clone()
        };
        *inputs = (acme, threshold);
        self.sender.send_replace(tls);
    }
}
