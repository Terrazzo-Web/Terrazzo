use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use notify::Watcher as _;
use tokio::sync::watch;
use tracing::debug;
use tracing::info;
use tracing::warn;

use super::Config;
use super::ConfigFile;
use super::DynConfig;
use super::ServerState;
use super::WatchConfig as _;
use super::io::ConfigFileError;
use crate::backend::auth::AuthConfig;
use crate::backend::cli::Cli;

impl Config {
    pub fn into_dyn(self, cli: Arc<Cli>) -> Arc<DynConfig> {
        let server = Arc::new(ServerState {
            config: (*self.server).clone(),
            auth: Arc::new(AuthConfig::new(&self.server)),
        });
        let (server_tx, server) = watch::channel(server);
        let (mesh_tx, mesh) = watch::channel(self.mesh.clone());
        let (letsencrypt_tx, letsencrypt) = watch::channel(self.letsencrypt.clone());
        let config = Arc::new(DynConfig {
            server,
            server_tx,
            mesh,
            mesh_tx,
            letsencrypt,
            letsencrypt_tx,
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
    match (serde_json::to_value(a), serde_json::to_value(b)) {
        (Ok(a), Ok(b)) => a == b,
        // Non-UTF8 paths cannot be represented in JSON. Conservatively publish
        // them rather than panicking or incorrectly suppressing an update.
        _ => false,
    }
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
            } else if server.password == old.password {
                Arc::new(
                    old.auth
                        .with_token_settings(server.token_lifetime, server.token_refresh),
                )
            } else {
                Arc::new(AuthConfig::new(&server))
            };
            self.server_tx.send_replace(Arc::new(ServerState {
                config: server,
                auth,
            }));
        }
        let mesh = new.mesh.as_deref().map(|v| Arc::new(v.clone()));
        self.mesh_tx.send_if_modified(|old| {
            if same(old, &mesh) {
                false
            } else {
                *old = mesh;
                true
            }
        });
        let acme = new.letsencrypt.as_deref().map(|v| Arc::new(v.clone()));
        self.letsencrypt_tx.send_if_modified(|old| {
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
    pub(in crate::backend) fn update_acme(
        &self,
        expected: &Arc<trz_gateway_server::server::acme::AcmeConfig<Arc<Path>>>,
        acme: trz_gateway_server::server::acme::AcmeConfig<Arc<Path>>,
        private_key: Option<&[u8]>,
    ) -> Result<bool, trz_gateway_server::server::acme::AcmeError> {
        use trz_gateway_server::server::acme::AcmeError;
        let mut state = self.state.lock().unwrap();
        let current = self.letsencrypt.get();
        let Some(current) = current else {
            return Ok(false);
        };
        if !Arc::ptr_eq(&current, expected) {
            return Ok(false);
        }
        if let Some(key) = private_key {
            acme.store_private_key(key)?;
        }
        let mut new = state.clone();
        new.0.letsencrypt = Some(Arc::new(acme));
        self.save(&new)
            .map_err(|error| AcmeError::Persist(error.to_string()))?;
        self.publish(&new);
        *state = new;
        Ok(true)
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
    // notify reports canonical directory paths (notably /private/var on macOS).
    // Resolve the parent, preserving the filename so atomic replacement works.
    let path = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .ok()
        .and_then(|parent| path.file_name().map(|name| parent.join(name)))
        .unwrap_or(path);
    let (events_tx, mut events) = tokio::sync::mpsc::unbounded_channel();
    let mut watcher = None;
    let mut watcher_enabled = false;
    // Start with no accepted fingerprint: an edit between startup and this task
    // must not be silently consumed.
    let mut accepted: Option<Vec<u8>> = None;
    let mut first = true;
    'reload: loop {
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
        tokio::pin!(wait);
        if !first {
            loop {
                tokio::select! {
                    _ = shutdown.changed() => break 'reload,
                    result = settings.changed() => {
                        if result.is_err() { break 'reload; }
                        let current = settings.borrow_and_update().clone();
                        if current.config_file_watcher != server.config_file_watcher
                            || current.config_file_poll_strategy != server.config_file_poll_strategy {
                            continue 'reload;
                        }
                        // Unrelated server changes must not reset the polling timer.
                    },
                    event = events.recv(), if watcher.is_some() => {
                        match event {
                            Some(Ok(event)) if is_relevant_config_event(&event, &path) => break,
                            Some(Err(error)) => warn!(%error, "Config watcher error"),
                            _ => {},
                        }
                    },
                    _ = &mut wait => break,
                }
            }
        }
        first = false;
        let Some(config) = config.upgrade() else {
            break;
        };
        reload_config_file(&config, &path, &cli, &mut accepted);
    }

    drop(watcher);
}

fn reload_config_file(config: &DynConfig, path: &Path, cli: &Cli, accepted: &mut Option<Vec<u8>>) {
    // Hold the mutation lock across read, merge, and publication. Runtime
    // writes and duplicate filesystem events cannot race this transaction.
    let mut state = config.state.lock().unwrap();
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            debug!(%error, "Cannot read config file");
            return;
        }
    };
    if accepted.as_ref() == Some(&bytes) {
        return;
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
        None => return,
    };
    let new = file.merge(&cli);
    config.publish(&new);
    *state = new;
    *accepted = Some(bytes);
}

fn is_relevant_config_event(event: &notify::Event, path: &Path) -> bool {
    matches!(
        event.kind,
        notify::EventKind::Create(_) | notify::EventKind::Modify(_)
    ) && event.paths.iter().any(|p| p == path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::cli::Action;

    fn cli(path: Option<PathBuf>) -> Arc<Cli> {
        Arc::new(Cli {
            action: Action::SetPassword,
            config_file: path,
            ..Cli::default()
        })
    }

    #[test]
    fn reload_keeps_channels_and_auth_keys_and_only_notifies_changed_values() {
        let cli = cli(None);
        let config = ConfigFile::default().merge(&cli).into_dyn(cli.clone());
        let mut server = config.server.subscribe();
        let mut mesh = config.mesh.subscribe();
        let auth = server.borrow_and_update().auth.clone();
        let new = ConfigFile::default().merge(&cli);
        config.publish(&new);
        assert!(!server.has_changed().unwrap());
        let mut new = new;
        Arc::make_mut(&mut new.0.server).terminal_shell = Some("/bin/sh".into());
        config.publish(&new);
        assert!(server.has_changed().unwrap());
        assert!(Arc::ptr_eq(&auth, &server.borrow_and_update().auth));
        assert_eq!(server.borrow().terminal_shell.as_deref(), Some("/bin/sh"));
        assert!(!mesh.has_changed().unwrap());
        let enabled: ConfigFile =
            toml::from_str("[mesh]\nclient_name = 'test'\ngateway_url = 'https://localhost:1234'")
                .unwrap();
        config.publish(&enabled.merge(&cli));
        assert!(mesh.has_changed().unwrap());
        assert_eq!(
            mesh.borrow_and_update().as_ref().unwrap().client_name,
            "test"
        );
        config.publish(&ConfigFile::default().merge(&cli));
        assert!(mesh.has_changed().unwrap());
        assert!(mesh.borrow().is_none());
    }

    #[test]
    fn late_subscribers_see_updates_and_startup_fields_stay_fixed() {
        let cli = cli(None);
        let config = ConfigFile::default().merge(&cli).into_dyn(cli.clone());
        let original_host = config.server.get().host.clone();
        let mut new = ConfigFile::default().merge(&cli);
        let server = Arc::make_mut(&mut new.0.server);
        server.host = "different-host".into();
        server.terminal_shell = Some("updated".into());
        config.publish(&new);
        let receiver = config.server.subscribe();
        assert_eq!(receiver.borrow().host, original_host);
        assert_eq!(receiver.borrow().terminal_shell.as_deref(), Some("updated"));
    }

    #[test]
    fn invalid_file_is_retryable_and_successful_reload_does_not_write_back() {
        let dir = std::env::temp_dir().join(format!("watch-config-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let _cleanup = scopeguard::guard(dir.clone(), |dir| {
            let _ = std::fs::remove_dir_all(dir);
        });
        let path = dir.join("config.toml");
        let cli = cli(Some(path.clone()));
        let config = ConfigFile::default().merge(&cli).into_dyn(cli.clone());
        let mut accepted = None;
        std::fs::write(&path, "[invalid").unwrap();
        reload_config_file(&config, &path, &cli, &mut accepted);
        assert!(accepted.is_none());
        let content =
            "# preserve this comment\n[server]\nports = []\nterminal-shell = 'reloaded'\n";
        toml::from_str::<ConfigFile>(content).unwrap();
        std::fs::write(&path, content).unwrap();
        reload_config_file(&config, &path, &cli, &mut accepted);
        assert_eq!(
            config.server.get().terminal_shell.as_deref(),
            Some("reloaded")
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
        assert!(accepted.is_some());
        let receiver = config.server.subscribe();
        reload_config_file(&config, &path, &cli, &mut accepted);
        assert!(!receiver.has_changed().unwrap());
    }

    #[test]
    fn password_save_failure_does_not_publish() {
        let cli = cli(Some(
            std::env::temp_dir()
                .join(uuid::Uuid::new_v4().to_string())
                .join("missing.toml"),
        ));
        let config = ConfigFile::default().merge(&cli).into_dyn(cli);
        let before = config.server.get();
        let password = super::super::types::Password {
            hash: vec![1],
            salt: vec![2],
            iterations: 1,
        };
        assert!(config.update_password(password).is_err());
        assert!(Arc::ptr_eq(&before, &config.server.get()));
    }
    #[test]
    fn duration_changes_preserve_signing_keys_without_a_password() {
        let cli = cli(None);
        let config = ConfigFile::default().merge(&cli).into_dyn(cli.clone());
        let token = config.server.get().auth.make_token().unwrap();
        let mut headers = terrazzo::http::HeaderMap::new();
        headers.insert(
            terrazzo::http::header::COOKIE,
            format!("{}={}", token.name(), token.value())
                .parse()
                .unwrap(),
        );
        let mut new = ConfigFile::default().merge(&cli);
        Arc::make_mut(&mut new.0.server).token_lifetime += std::time::Duration::from_secs(100);
        config.publish(&new);
        assert!(config.server.get().auth.validate(&headers).is_ok());
    }

    #[test]
    fn obsolete_acme_results_cannot_write_keys_or_reenable_acme() {
        let cli = cli(None);
        let file: ConfigFile = toml::from_str(
            r#"
[letsencrypt]
environment = "Staging"
contact = "mailto:test@example.com"
domains = ["example.com"]
"#,
        )
        .unwrap();
        let config = file.merge(&cli).into_dyn(cli.clone());
        let expected = config.letsencrypt.get().unwrap();
        config.publish(&ConfigFile::default().merge(&cli));
        let mut stale = (*expected).clone();
        let path = std::env::temp_dir().join(format!("stale-acme-{}", uuid::Uuid::new_v4()));
        stale.private_key = path.clone().into();
        assert!(
            !config
                .update_acme(&expected, stale, Some(b"must not be written"))
                .unwrap()
        );
        assert!(!path.exists());
        assert!(config.letsencrypt.get().is_none());
    }

    #[tokio::test]
    async fn atomic_file_replacement_and_switching_to_polling_and_shutdown() {
        let dir = std::env::temp_dir().join(format!("watch-config-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let _cleanup = scopeguard::guard(dir.clone(), |dir| {
            let _ = std::fs::remove_dir_all(dir);
        });
        let path = dir.join("config.toml");
        let cli = cli(Some(path.clone()));
        let config = ConfigFile::default().merge(&cli).into_dyn(cli.clone());
        let mut receiver = config.server.subscribe();
        std::fs::write(&path, "[server]\nports = []\nterminal-shell = 'ready'\n").unwrap();
        let reload = tokio::spawn(run_config_reload_tasks(
            path.clone(),
            cli.clone(),
            Arc::downgrade(&config),
            config.shutdown.subscribe(),
        ));
        async fn next(receiver: &mut watch::Receiver<Arc<ServerState>>, shell: &str) {
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                loop {
                    receiver.changed().await.unwrap();
                    if receiver.borrow_and_update().terminal_shell.as_deref() == Some(shell) {
                        break;
                    }
                }
            })
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "Timed out waiting for shell {shell}, current {:?}",
                    receiver.borrow().terminal_shell
                )
            });
        }
        next(&mut receiver, "ready").await;
        let replacement = dir.join("replacement.toml");
        std::fs::write(
            &replacement,
            "[server]\nports = []\nterminal-shell = 'watcher'\n",
        )
        .unwrap();
        std::fs::rename(&replacement, &path).unwrap();
        next(&mut receiver, "watcher").await;
        std::fs::write(&path, "[server]\nports = []\nterminal-shell = 'polling'\nconfig_file_watcher = false\n[server.config_file_poll_strategy]\nfixed = '10ms'\n").unwrap();
        next(&mut receiver, "polling").await;
        std::fs::write(&path, "[server]\nports = []\nterminal-shell = 'polled'\nconfig_file_watcher = false\n[server.config_file_poll_strategy]\nfixed = '10ms'\n").unwrap();
        next(&mut receiver, "polled").await;
        config.shutdown();
        tokio::time::timeout(std::time::Duration::from_secs(5), reload)
            .await
            .unwrap()
            .unwrap();
    }
}
