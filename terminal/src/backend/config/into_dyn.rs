use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;

use notify::RecommendedWatcher;
use notify::Watcher as _;
use terrazzo::autoclone;
use tracing::Instrument as _;
use tracing::debug;
use tracing::info;
use tracing::info_span;
use tracing::trace;
use tracing::warn;
use trz_gateway_common::dynamic_config::DynamicConfig;
use trz_gateway_common::dynamic_config::has_diff::DiffArc;
use trz_gateway_common::dynamic_config::has_diff::DiffOption;
use trz_gateway_common::dynamic_config::has_diff::HasDiff;
use trz_gateway_common::dynamic_config::mode::RO;
use trz_gateway_server::server::acme::AcmeConfig;
use trz_gateway_server::server::acme::DynamicAcmeConfig;

use super::Config;
use super::ConfigFile;
use super::ConfigImpl;
use super::DynConfig;
use super::mesh::DynamicMeshConfig;
use super::server::DynamicServerConfig;
use crate::backend::cli::Cli;
use crate::backend::config::server::ServerConfig;

impl Config {
    #[autoclone]
    pub fn into_dyn(self, cli: Arc<Cli>) -> DiffArc<DynConfig> {
        let config = Arc::from(DynamicConfig::from(DiffArc::from(self)));
        let server = DynamicServerConfig::from(config.derive(
            |config| config.server.clone(),
            |config, server_config| {
                if HasDiff::is_same(&config.server, server_config) {
                    return None;
                }
                debug!("Updated server config");
                Some(DiffArc::from(Config::from(ConfigImpl {
                    server: server_config.clone(),
                    ..(**config).clone()
                })))
            },
        ));
        let mesh = DynamicMeshConfig::from(config.derive(
            |config| config.mesh.clone(),
            |config, mesh_config| {
                if DiffOption::is_same(&config.mesh, mesh_config) {
                    return None;
                }
                debug!("Updated mesh config");
                Some(DiffArc::from(Config::from(ConfigImpl {
                    mesh: mesh_config.clone(),
                    ..(**config).clone()
                })))
            },
        ));
        let letsencrypt = DynamicAcmeConfig::from(config.derive(
            |config| config.letsencrypt.clone(),
            |config, letsencrypt| {
                if DiffOption::is_same(&config.letsencrypt, letsencrypt) {
                    return None;
                }
                debug!("Updated letsencrypt config");
                Some(DiffArc::from(Config::from(ConfigImpl {
                    letsencrypt: letsencrypt.clone(),
                    ..(**config).clone()
                })))
            },
        ));
        let config_file_path = cli.config_file.to_owned();
        let dyn_config_file: Arc<DynamicConfig<(), RO>> = config.view(move |config| {
            autoclone!(config_file_path);
            if let Some(config_file_path) = config_file_path.as_deref() {
                let () = config
                    .to_config_file()
                    .save(config_file_path)
                    .inspect(|()| info!("Saved config file {config_file_path}"))
                    .unwrap_or_else(|error| warn!("Failed to save {config_file_path}: {error}"));
            }
        });
        let config = DiffArc::from(DynConfig {
            server,
            mesh,
            letsencrypt,
            config,
            dyn_config_file,
        });
        if cli.action != crate::backend::cli::Action::SetPassword
            && let Some(config_file_path) = &config_file_path
        {
            tokio::spawn(run_config_reload_tasks(
                config_file_path.to_owned(),
                cli,
                config.clone(),
            ));
        }
        return config;
    }
}

#[derive(Default)]
struct ReloadState {
    last_modified: Mutex<Option<SystemTime>>,
}

async fn run_config_reload_tasks(
    config_file_path: String,
    cli: Arc<Cli>,
    config: DiffArc<DynConfig>,
) {
    let span = info_span!("Config file live reload", config_file_path);
    async move {
        let state = Arc::new(ReloadState {
            last_modified: Mutex::new(config_file_last_modified(Path::new(&config_file_path))),
        });
        let watcher_enabled = config.with(|config| config.server.config_file_watcher);
        let polling_enabled =
            config.with(|config| config.server.config_file_poll_strategy.is_some());

        if watcher_enabled {
            match spawn_config_file_watcher(
                PathBuf::from(&config_file_path),
                cli.clone(),
                config.clone(),
                state.clone(),
            ) {
                Ok(()) => info!("Started config file watcher"),
                Err(error) => warn!("Failed to start config file watcher: {error}"),
            }
        } else {
            info!("Config file watcher is disabled");
        }

        if polling_enabled {
            tokio::spawn(poll_config_file(
                config_file_path.clone(),
                cli.clone(),
                config.clone(),
                state.clone(),
            ));
            info!("Started config file polling");
        } else {
            info!("Config file polling is disabled");
        }

        if !watcher_enabled && !polling_enabled {
            warn!("Config file live reload is disabled");
        }
    }
    .instrument(span)
    .await
}

fn spawn_config_file_watcher(
    config_file_path: PathBuf,
    cli: Arc<Cli>,
    config: DiffArc<DynConfig>,
    state: Arc<ReloadState>,
) -> notify::Result<()> {
    let watch_path = config_file_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_owned();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |event| {
        let _ = tx.send(event);
    })?;
    watcher.watch(&watch_path, notify::RecursiveMode::NonRecursive)?;

    tokio::spawn(async move {
        let _watcher = watcher;
        while let Some(event) = rx.recv().await {
            let event = match event {
                Ok(event) => event,
                Err(error) => {
                    warn!("Config file watcher failed: {error}");
                    continue;
                }
            };

            if !is_relevant_config_event(&event, &config_file_path) {
                trace!(?event, "Ignoring unrelated config watcher event");
                continue;
            }

            reload_config_file("watcher", &config_file_path, &cli, &config, &state).await;
        }
    });

    Ok(())
}

fn is_relevant_config_event(event: &notify::Event, config_file_path: &Path) -> bool {
    match event.kind {
        notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {}
        _ => return false,
    }

    event.paths.iter().any(|path| path == config_file_path)
}

async fn poll_config_file(
    config_file_path: String,
    cli: Arc<Cli>,
    config: DiffArc<DynConfig>,
    state: Arc<ReloadState>,
) {
    let span = info_span!("Polling config file", config_file_path);
    let config_file_path = PathBuf::from(config_file_path);
    async move {
        loop {
            let Some(mut retry_strategy) =
                config.with(|config| config.server.config_file_poll_strategy.clone())
            else {
                info!("Config file polling has been disabled");
                return;
            };
            retry_strategy.wait().await;
            debug!("Polling config file");
            reload_config_file("poll", &config_file_path, &cli, &config, &state).await;
        }
    }
    .instrument(span)
    .await
}

async fn reload_config_file(
    trigger: &'static str,
    config_file_path: &Path,
    cli: &Arc<Cli>,
    config: &DiffArc<DynConfig>,
    state: &ReloadState,
) {
    let Some(modified) = config_file_last_modified(config_file_path) else {
        debug!(trigger, "Config file is missing or has no timestamp");
        return;
    };

    {
        let mut last_modified = state.last_modified.lock().unwrap();
        if last_modified.as_ref() == Some(&modified) {
            trace!(
                trigger,
                "Config file reload skipped because timestamp is unchanged"
            );
            return;
        }
        *last_modified = Some(modified);
    }

    info!(trigger, "Config file timestamp has changed");

    let new_config_file = match ConfigFile::load(config_file_path) {
        Ok(new_config_file) => new_config_file,
        Err(error) => {
            warn!("Failed to load config file: {error}");
            return;
        }
    };
    let new = new_config_file.merge(cli);
    apply_server_config(config, &new.server);
    apply_letsencrypt_config(config, &new.letsencrypt);
}

fn config_file_last_modified(config_file_path: &Path) -> Option<SystemTime> {
    let metadata = std::fs::metadata(config_file_path).ok()?;
    metadata.modified().ok()
}

fn apply_server_config(config: &DiffArc<DynConfig>, new: &ServerConfig) {
    let is_server_changed = config.server.try_set(|old| {
        let mut result = Err(());
        fn get_or_init<'t>(
            old: &ServerConfig,
            result: &'t mut Result<ServerConfig, ()>,
        ) -> &'t mut ServerConfig {
            match result {
                Ok(r) => r,
                Err(()) => {
                    *result = Ok(old.clone());
                    return result.as_mut().unwrap();
                }
            }
        }
        if new.password != old.password {
            info!("Changed: password");
            let result = get_or_init(old, &mut result);
            result.password = new.password.clone();
        }
        if new.token_lifetime != old.token_lifetime {
            info!("Changed: token_lifetime");
            let result = get_or_init(old, &mut result);
            result.token_lifetime = new.token_lifetime;
        }
        if new.token_refresh != old.token_refresh {
            info!("Changed: token_refresh");
            let result = get_or_init(old, &mut result);
            result.token_refresh = new.token_refresh;
        }
        if new.config_file_watcher != old.config_file_watcher {
            info!("Changed: config_file_watcher");
            let result = get_or_init(old, &mut result);
            result.config_file_watcher = new.config_file_watcher;
        }
        if new.config_file_poll_strategy != old.config_file_poll_strategy {
            info!("Changed: config_file_poll_strategy");
            let result = get_or_init(old, &mut result);
            result.config_file_poll_strategy = new.config_file_poll_strategy.clone();
        }

        return result.map(DiffArc::from);
    });
    match is_server_changed {
        Ok(()) => info!("ServerConfig has changed"),
        Err(()) => debug!("ServerConfig hasn't changed"),
    }
}

fn apply_letsencrypt_config(config: &DiffArc<DynConfig>, new: &DiffOption<DiffArc<AcmeConfig>>) {
    let is_letsencrypt_changed =
        config
            .letsencrypt
            .try_set(|old: &DiffOption<DiffArc<AcmeConfig>>| {
                match (old.as_deref(), new.as_deref()) {
                    (None, None) => false,
                    (None, Some(_)) | (Some(_), None) => true,
                    (Some(old), Some(new)) => old != new,
                }
                .then(|| new.clone())
                .ok_or(())
            });
    match is_letsencrypt_changed {
        Ok(()) => info!("Let's Encrypt config has changed"),
        Err(()) => debug!("Let's Encrypt config hasn't changed"),
    }
}
