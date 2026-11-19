use std::path::{Path, PathBuf};
use std::sync::Arc;

use notify::Watcher as _;
use tokio::sync::watch;
use tracing::{debug, info, warn};

use super::io::ConfigFileError;
use super::{Config, ConfigFile, DynConfig, ServerState, WatchConfig as _};
use crate::backend::auth::AuthConfig;
use crate::backend::cli::Cli;

impl Config {
    pub fn into_dyn(self, cli: Arc<Cli>) -> Arc<DynConfig> {
        let server = Arc::new(ServerState {
            config: (*self.server).clone(),
            auth: Arc::new(AuthConfig::new(&self.server)),
        });
        let config = Arc::new(DynConfig {
            server: watch::channel(server).0,
            mesh: watch::channel(self.mesh.as_deref().map(|v| Arc::new(v.clone()))).0,
            letsencrypt: watch::channel(self.letsencrypt.as_deref().map(|v| Arc::new(v.clone()))).0,
            state: std::sync::Mutex::new(self),
            config_file: cli.config_file.clone(),
            shutdown: watch::channel(false).0,
            tls: std::sync::OnceLock::new(),
        });
        if cli.action != crate::backend::cli::Action::SetPassword {
            if let Some(path) = config.config_file.clone() {
                tokio::spawn(run_config_reload_tasks(
                    path,
                    cli,
                    Arc::downgrade(&config),
                    config.shutdown.subscribe(),
                ));
            }
        }
        config
    }
}

/// Compare contents, never allocation identities. All config types serialize to JSON.
fn same<T: serde::Serialize>(a: &T, b: &T) -> bool {
    serde_json::to_value(a).expect("serialize config")
        == serde_json::to_value(b).expect("serialize config")
}

impl DynConfig {
    fn save(&self, config: &Config) -> Result<(), ConfigFileError> {
        if let Some(path) = &self.config_file {
            config.to_config_file().save(path)?;
        }
        Ok(())
    }

    /// Caller holds state, serializing reloads with password and ACME updates.
    fn publish(&self, new: &Config) {
        let old = self.server.get();
        let mut server = (*new.server).clone();
        // These settings have startup side effects, not live readers.
        macro_rules! startup {
            ($($field:ident),* $(,)?) => { $(
                if !same(&server.$field, &old.$field) {
                    info!(field = stringify!($field), "Config change requires restart");
                }
                server.$field = old.$field.clone();
            )* };
        }
        startup!(
            host,
            ports,
            tiles_file,
            set_current_endpoint,
            pidfile,
            private_root_ca,
            p2p_registration
        );
        if !same(&server, &old.config) {
            let auth = if server.password == old.password
                && server.token_lifetime == old.token_lifetime
                && server.token_refresh == old.token_refresh
            {
                old.auth.clone()
            } else {
                Arc::new(AuthConfig::new(&server))
            };
            self.server.send_replace(Arc::new(ServerState {
                config: server,
                auth,
            }));
        }
        let mesh = new.mesh.as_deref().map(|v| Arc::new(v.clone()));
        self.mesh.send_if_modified(|old| {
            if same(old, &mesh) {
                false
            } else {
                *old = mesh;
                true
            }
        });
        let acme = new.letsencrypt.as_deref().map(|v| Arc::new(v.clone()));
        self.letsencrypt.send_if_modified(|old| {
            if same(old, &acme) {
                false
            } else {
                *old = acme;
                true
            }
        });
        if let Some(tls) = self.tls.get() {
            tls.refresh(self);
        }
    }

    pub(super) fn update_password(
        &self,
        password: super::types::Password,
    ) -> Result<(), ConfigFileError> {
        let mut state = self.state.lock().unwrap();
        let mut new = state.clone();
        let mut server = (*new.server).clone();
        server.password = Some(password);
        new.0.server = server.into();
        self.save(&new)?;
        self.publish(&new);
        *state = new;
        Ok(())
    }

    /// Commit only results produced for the still-current ACME configuration.
    pub(super) fn update_acme(
        &self,
        expected: &Arc<trz_gateway_server::server::acme::AcmeConfig<Arc<Path>>>,
        update: impl FnOnce(&mut trz_gateway_server::server::acme::AcmeConfig<Arc<Path>>),
    ) {
        let mut state = self.state.lock().unwrap();
        let current = self.letsencrypt.get();
        let Some(current) = current else {
            return;
        };
        if !Arc::ptr_eq(&current, expected) {
            return;
        }
        let mut acme = (*current).clone();
        update(&mut acme);
        let mut new = state.clone();
        new.0.letsencrypt = Some(Arc::new(acme));
        if let Err(error) = self.save(&new) {
            warn!(%error, "Failed to save ACME update");
            return;
        }
        self.publish(&new);
        *state = new;
    }
}

/// One coordinator owns the watcher and polling timer. No detached reload workers.
async fn run_config_reload_tasks(
    path: PathBuf,
    cli: Arc<Cli>,
    config: std::sync::Weak<DynConfig>,
    mut shutdown: watch::Receiver<bool>,
) {
    let Some(initial) = config.upgrade() else {
        return;
    };
    let mut settings = initial.server.subscribe();
    drop(initial);
    let (events_tx, mut events) = tokio::sync::mpsc::unbounded_channel();
    let mut watcher = None;
    let mut watcher_enabled = false;
    // Start with no accepted fingerprint: an edit between startup and this task
    // must not be silently consumed.
    let mut accepted: Option<Vec<u8>> = None;
    loop {
        if *shutdown.borrow() {
            break;
        }
        let server = settings.borrow_and_update().clone();
        if server.config_file_watcher != watcher_enabled {
            watcher = None;
            watcher_enabled = server.config_file_watcher;
            if watcher_enabled {
                let tx = events_tx.clone();
                match notify::recommended_watcher(move |event| {
                    let _ = tx.send(event);
                })
                .and_then(|mut w| {
                    let parent = path
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .unwrap_or(Path::new("."));
                    w.watch(parent, notify::RecursiveMode::NonRecursive)?;
                    Ok(w)
                }) {
                    Ok(w) => {
                        watcher = Some(w);
                        info!("Started config file watcher");
                    }
                    Err(error) => warn!(%error, "Failed to start config file watcher"),
                }
            }
        }
        let mut strategy = server.config_file_poll_strategy.clone();
        let wait = async {
            match &mut strategy {
                Some(strategy) => strategy.wait().await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            _ = shutdown.changed() => break,
            result = settings.changed() => { if result.is_err() { break; } continue; },
            event = events.recv(), if watcher.is_some() => {
                match event {
                    Some(Ok(event)) if is_relevant_config_event(&event, &path) => {},
                    Some(Err(error)) => { warn!(%error, "Config watcher error"); continue; },
                    _ => continue,
                }
            },
            _ = wait => {},
        }
        let Some(config) = config.upgrade() else {
            break;
        };
        // Hold the mutation lock across read, merge, and publication. Runtime
        // writes and duplicate filesystem events cannot race this transaction.
        let mut state = config.state.lock().unwrap();
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                debug!(%error, "Cannot read config file");
                continue;
            }
        };
        if accepted.as_ref() == Some(&bytes) {
            continue;
        }
        let file = match std::str::from_utf8(&bytes).ok().and_then(|text| {
            if text.is_empty() {
                Some(ConfigFile::default())
            } else {
                toml::from_str::<ConfigFile>(text)
                    .inspect_err(|error| warn!(%error, "Invalid config file"))
                    .ok()
            }
        }) {
            Some(file) => file,
            None => continue,
        };
        let new = file.merge(&cli);
        config.publish(&new);
        *state = new;
        accepted = Some(bytes);
    }
    drop(watcher);
}

fn is_relevant_config_event(event: &notify::Event, path: &Path) -> bool {
    matches!(
        event.kind,
        notify::EventKind::Create(_) | notify::EventKind::Modify(_)
    ) && event.paths.iter().any(|p| p == path)
}
