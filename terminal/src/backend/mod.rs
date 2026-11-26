#![cfg(feature = "server")]
use std::ops::ControlFlow;
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::SystemTime;

use clap::Parser as _;
use futures::future::Either;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use scopeguard::defer;
use tracing::debug;
use tracing::info;
use tracing::warn;
use trz_gateway_client::client::AuthCode;
use trz_gateway_client::client::Client;
use trz_gateway_client::client::NewClientError;
use trz_gateway_client::client::connect::ConnectError;
use trz_gateway_client::load_client_certificate::make_client_certificate;
use trz_gateway_client::load_client_certificate::store_client_certificate;
use trz_gateway_client::tunnel_config::TunnelConfig;
use trz_gateway_common::crypto_provider::crypto_provider;
use trz_gateway_common::handle::ServerHandle;
use trz_gateway_common::handle::ServerStopError;
use trz_gateway_common::notify_once;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::x509::time::asn1_to_system_time;
use trz_gateway_server::server::GatewayError;
use trz_gateway_server::server::Server as GatewayServer;

use self::agent::AgentTunnelConfig;
use self::cli::Action;
use self::cli::Cli;
use self::config::Config;
use self::config::ConfigFile;
use self::config::DynConfig;
use self::config::io::ConfigFileError;
use self::config::kill::KillServerError;
use self::config::password::SetPasswordError;
use self::daemonize::DaemonizeServerError;
use self::root_ca_config::PrivateRootCaError;
use self::server_config::TerminalBackendServer;
use self::tls_config::TlsConfigError;
use crate::backend::config::WatchConfig as _;

mod agent;
pub mod auth;
mod cli;
pub mod client_service;
pub mod config;
mod daemonize;
pub mod protos;
mod root_ca_config;
mod run_server_graph;
mod server_config;
pub mod throttling_stream;
mod tls_config;

const HOST: &str = "localhost";
const PORT: u16 = if cfg!(debug_assertions) { 3000 } else { 3001 };
const MIN_CERTIFICATE_RENEWAL_DELAY: Duration = if cfg!(debug_assertions) {
    Duration::from_secs(5)
} else {
    Duration::from_secs(60 * 5)
};

pub struct Server {
    gateway: Arc<GatewayServer>,
    config: Arc<DynConfig>,
}

impl Server {
    pub fn new(gateway: Arc<GatewayServer>, config: Arc<DynConfig>) -> Self {
        Self { gateway, config }
    }

    pub fn config(&self) -> &Arc<DynConfig> {
        &self.config
    }
}

impl Deref for Server {
    type Target = GatewayServer;

    fn deref(&self) -> &Self::Target {
        &self.gateway
    }
}

pub fn run_server() -> Result<(), RunServerError> {
    crypto_provider();
    let cli = {
        let mut cli = Cli::parse();
        if let Some(config_file) = &mut cli.config_file
            && Path::new(config_file).is_relative()
        {
            *config_file = terrazzo_home().join(&config_file)
        }
        cli
    };

    if cli.password_stdin && cli.action != Action::SetPassword {
        return Err(RunServerError::PasswordStdinRequiresSetPassword);
    }

    #[cfg(feature = "debug")]
    if cli.action == Action::ListAssets {
        crate::assets::install::install_assets();
        terrazzo::static_assets::echo_asset_dependencies(env!("CARGO_MANIFEST_DIR"));
        return Ok(());
    }

    let config = if let Some(path) = cli.config_file.as_deref() {
        ConfigFile::load(path)?
    } else {
        ConfigFile::default()
    }
    .merge(&cli);

    #[cfg(debug_assertions)]
    println!("Config: {config:#?}");

    if cli.action == Action::Stop {
        return Ok(config.server.kill()?);
    }

    if cli.action == Action::Restart {
        config.server.kill()?;
    }

    std::env::set_current_dir(home()).map_err(RunServerError::SetCurrentDir)?;
    if cli.action == Action::Start {
        self::daemonize::daemonize(&config.server)?;
    }

    return run_server_async(cli, config);
}

#[tokio::main]
async fn run_server_async(cli: Cli, config: Config) -> Result<(), RunServerError> {
    let cli = Arc::new(cli);
    #[cfg(feature = "logs-panel")]
    {
        crate::logs::init_tracing()?;
    }
    #[cfg(debug_assertions)]
    {
        println!("server_fn paths:");
        for (m, p) in server_fn::axum::server_fn_paths() {
            println!("\t{m} {p}");
        }
        println!("server_fn paths END");
    }
    let config = config.into_dyn(cli.clone());
    let server_config = config.server.clone();
    if cli.action == Action::SetPassword {
        return Ok(config.set_password(cli.password_stdin)?);
    }

    crate::tiles::api::state::initialize(server_config.with(|config| config.tiles_file.clone()))
        .await
        .map_err(RunServerError::Tiles)?;

    let result = run_server_graph::Run { cli, config }.run().await;
    crate::tiles::api::state::wait_for_pending_tile_saves().await;
    result
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum RunServerError {
    #[error("Failed to initialize tile persistence: {0}")]
    Tiles(crate::tiles::api::state::TilesStateError),

    #[error("[{n}] --password-stdin requires --action set-password", n = self.name())]
    PasswordStdinRequiresSetPassword,

    #[error("[{n}] {0}", n = self.name())]
    KillServer(#[from] KillServerError),

    #[error("[{n}] {0}", n = self.name())]
    ConfigFile(#[from] ConfigFileError),

    #[error("[{n}] {0}", n = self.name())]
    SetPassword(#[from] SetPasswordError),

    #[error("[{n}] {0}", n = self.name())]
    PrivateRootCa(#[from] PrivateRootCaError),

    #[error("[{n}] {0}", n = self.name())]
    TlsConfig(#[from] TlsConfigError),

    #[error("[{n}] {0}", n = self.name())]
    Daemonize(#[from] DaemonizeServerError),

    #[error("[{n}] {0}", n = self.name())]
    Server(#[from] GatewayError<TerminalBackendServer>),

    #[error("[{n}] {0}", n = self.name())]
    SetCurrentDir(std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    Signal(std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    Stop(#[from] ServerStopError),

    #[error("[{n}] {0}", n = self.name())]
    RunClient(#[from] RunClientError),

    #[error("[{n}] {0}", n = self.name())]
    EnableTracing(#[from] EnableTracingError),
}

#[cfg(feature = "logs-panel")]
#[doc(hidden)]
type EnableTracingError = crate::logs::EnableTracingError;

#[cfg(not(feature = "logs-panel"))]
#[doc(hidden)]
#[derive(thiserror::Error, Debug)]
#[error("{self:?}")]
pub struct EnableTracingError;

async fn run_client_async(
    cli: Arc<Cli>,
    config: Arc<DynConfig>,
    server: Arc<Server>,
) -> Result<ServerHandle<()>, RunClientError> {
    let (shutdown_rx, terminated_tx, handle) = ServerHandle::new("Dynamic Client");
    let auth_code = AuthCode::from(cli.auth_code.clone());
    let mut mesh_rx = config.mesh.subscribe();
    let (restart_tx, mut restart_rx) =
        tokio::sync::mpsc::unbounded_channel::<Arc<self::config::mesh::MeshConfig>>();
    tokio::spawn(async move {
        tokio::pin!(shutdown_rx);
        loop {
            let mesh = mesh_rx.borrow_and_update().clone();
            let Some(mesh) = mesh else {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    result = mesh_rx.changed() => { if result.is_err() { break; } },
                }
                continue;
            };
            let auth_code = Arc::new(Mutex::new(auth_code.clone()));
            let initialize = mesh.retry_strategy.clone().process(|| async {
                match AgentTunnelConfig::new(auth_code.clone(), &mesh, &server).await {
                    Ok(config) => ControlFlow::Break(Ok(config)),
                    Err(error) if !error.is_retryable() => ControlFlow::Break(Err(error)),
                    Err(error) => ControlFlow::Continue(error),
                }
            });
            let initialized = tokio::select! {
                _ = &mut shutdown_rx => break,
                result = mesh_rx.changed() => { if result.is_err() { break; } continue; },
                result = initialize => result,
            };
            let client = match initialized
                .map_err(|error| error.to_string())
                .and_then(|agent| {
                    let agent = Arc::new(agent);
                    Client::new(agent.clone())
                        .map(|client| (agent, client))
                        .map_err(|error| error.to_string())
                }) {
                Ok(client) => client,
                Err(error) => {
                    warn!(%error, "Failed to initialize mesh client");
                    tokio::select! {
                        _ = &mut shutdown_rx => break,
                        result = mesh_rx.changed() => { if result.is_err() { break; } continue; },
                    }
                }
            };
            let (agent, client) = client;
            let running = tokio::select! {
                _ = &mut shutdown_rx => break,
                result = mesh_rx.changed() => { if result.is_err() { break; } continue; },
                result = client.run() => result,
            };
            let client_handle = match running {
                Ok(handle) => handle,
                Err(error) => {
                    warn!(%error, "Failed to start mesh client");
                    tokio::select! {
                        _ = &mut shutdown_rx => break,
                        result = mesh_rx.changed() => { if result.is_err() { break; } continue; },
                    }
                }
            };
            let (abort_tx, abort_rx) = notify_once::channel();
            let renewal = tokio::spawn(schedule_client_certificate_renewal(
                abort_rx,
                mesh.clone(),
                restart_tx.clone(),
                mesh.client_certificate_renewal,
                agent,
            ));
            let stop = loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break true,
                    result = mesh_rx.changed() => break result.is_err(),
                    Some(expected) = restart_rx.recv() => {
                        if Arc::ptr_eq(&expected, &mesh) { break false; }
                    }
                }
            };
            let _ = abort_tx.send(());
            renewal.abort();
            let _ = renewal.await;
            if let Err(error) = client_handle
                .stop("Mesh configuration updated or shutdown")
                .await
            {
                warn!(%error, "Failed to stop mesh client");
            }
            if stop {
                break;
            }
        }
        let _ = terminated_tx.send(());
    });
    Ok(handle)
}

async fn schedule_client_certificate_renewal(
    abort_client_rx: notify_once::Receiver<()>,
    mesh: Arc<self::config::mesh::MeshConfig>,
    restart: tokio::sync::mpsc::UnboundedSender<Arc<self::config::mesh::MeshConfig>>,
    client_certificate_renewal: Duration,
    client_config: impl TunnelConfig,
) {
    debug!("Start");
    defer!(debug!("Canceled"));
    loop {
        let client_certificate = client_config.client_certificate();
        let expiration = if let Ok(Ok(expiration)) = client_certificate
            .certificate()
            .map(|certificate| asn1_to_system_time(certificate.certificate.not_after()))
        {
            expiration
        } else {
            debug!("Failed to parse certificat not_after date");
            SystemTime::UNIX_EPOCH
        };
        let now = SystemTime::now();
        let renew_in = if expiration <= now + client_certificate_renewal {
            info!(
                "Certificate is already expiring expiration:{expiration} <= now:{now} + client_certificate_renewal:{client_certificate_renewal}",
                expiration = humantime::format_rfc3339(expiration),
                now = humantime::format_rfc3339(now),
                client_certificate_renewal = humantime::format_duration(client_certificate_renewal),
            );
            MIN_CERTIFICATE_RENEWAL_DELAY
        } else {
            let renew_at = expiration - client_certificate_renewal;
            renew_at
                .duration_since(now)
                .unwrap_or(MIN_CERTIFICATE_RENEWAL_DELAY)
        };

        debug!(
            "Renewing client certificate in {}",
            humantime::format_duration(renew_in)
        );
        match futures::future::select(
            abort_client_rx.clone(),
            Box::pin(tokio::time::sleep(renew_in)),
        )
        .await
        {
            Either::Left((_abort, _sleep)) => return,
            Either::Right(((), _abort_client_rx)) => {}
        }

        info!("Renewing client certificate");
        let auth_code = client_config.current_auth_code().lock().unwrap().clone();
        let Ok(new_certificate) = make_client_certificate(&client_config, auth_code)
            .await
            .inspect_err(|error| warn!("Failed to renew client certificate: {error}"))
        else {
            continue;
        };
        let renewed =
            store_client_certificate(mesh.client_certificate_paths().as_ref(), new_certificate)
                .inspect(|_| info!("Renewed the client certificate"))
                .inspect_err(|error| warn!("Failed to store new client certificate: {error}"))
                .is_ok();
        if renewed {
            let _ = restart.send(mesh.clone());
            return;
        }
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum RunClientError {
    #[error("[{n}] Not running Gateway Client", n = self.name())]
    ClientNotEnabled,

    #[error("[{n}] {0}", n = self.name())]
    NewClient(#[from] NewClientError<Arc<AgentTunnelConfig>>),

    #[error("[{n}] {0}", n = self.name())]
    RunClientError(#[from] ConnectError),

    #[error("[{n}] {0}", n = self.name())]
    Aborted(String),
}

fn terrazzo_home() -> &'static Path {
    static TERRAZZO_HOME: OnceLock<PathBuf> = OnceLock::new();
    TERRAZZO_HOME.get_or_init(|| home().join(".terrazzo"))
}

fn home() -> &'static Path {
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| std::env::var("HOME").expect("HOME").into())
}
