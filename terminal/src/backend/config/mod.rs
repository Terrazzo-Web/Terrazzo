use std::fmt::Debug;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;
use tokio::sync::watch;
use trz_gateway_server::server::acme::AcmeConfig;

use self::mesh::MeshConfig;
use self::server::ServerConfig;
use self::types::ConfigFileTypes;
use self::types::ConfigTypes;
use self::types::RuntimeTypes;

mod into_dyn;
pub(in crate::backend) mod io;
pub(in crate::backend) mod kill;
mod merge;
pub mod mesh;
pub mod p2p;
pub(in crate::backend) mod password;
pub(in crate::backend) mod pidfile;
pub mod server;
mod tls;
pub(in crate::backend) mod types;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConfigFile(ConfigImpl<ConfigFileTypes>);

impl Deref for ConfigFile {
    type Target = ConfigImpl<ConfigFileTypes>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// One runtime allocation. Channel identities never change during reload.
pub struct DynConfig {
    pub server: watch::Receiver<Arc<ServerState>>,
    server_tx: watch::Sender<Arc<ServerState>>,
    pub mesh: watch::Receiver<Option<Arc<MeshConfig>>>,
    mesh_tx: watch::Sender<Option<Arc<MeshConfig>>>,
    pub letsencrypt: watch::Receiver<Option<Arc<AcmeConfig<Arc<Path>>>>>,
    letsencrypt_tx: watch::Sender<Option<Arc<AcmeConfig<Arc<Path>>>>>,
    state: std::sync::Mutex<Config>,
    config_file: Option<std::path::PathBuf>,
    tls: std::sync::OnceLock<tls::RuntimeTls>,
    shutdown: watch::Sender<bool>,
}

/// Authentication inputs and signing keys are published in the same snapshot.
pub struct ServerState {
    config: ServerConfig,
    pub auth: Arc<crate::backend::auth::AuthConfig>,
}

impl Deref for ServerState {
    type Target = ServerConfig;
    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

/// Short, synchronous reads; no watch borrow escapes into asynchronous work.
pub trait WatchConfig<T> {
    fn get(&self) -> T
    where
        T: Clone;
    fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R;
    fn subscribe(&self) -> watch::Receiver<T>;
}

impl<T> WatchConfig<T> for watch::Receiver<T> {
    fn get(&self) -> T
    where
        T: Clone,
    {
        self.borrow().clone()
    }
    fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.borrow())
    }
    fn subscribe(&self) -> watch::Receiver<T> {
        let mut receiver = self.clone();
        receiver.mark_unchanged();
        receiver
    }
}

impl DynConfig {
    pub fn shutdown(&self) {
        self.shutdown.send_replace(true);
    }
}

#[derive(Clone, Debug)]
pub struct Config(ConfigImpl<RuntimeTypes>);

impl Deref for Config {
    type Target = ConfigImpl<RuntimeTypes>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<ConfigImpl<RuntimeTypes>> for Config {
    fn from(value: ConfigImpl<RuntimeTypes>) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigImpl<T: ConfigTypes> {
    pub server: Arc<ServerConfig<T>>,
    pub mesh: Option<Arc<MeshConfig<T>>>,
    pub letsencrypt: Option<Arc<AcmeConfig<T::Path>>>,
}

impl Default for ConfigImpl<ConfigFileTypes> {
    fn default() -> Self {
        Self {
            server: Default::default(),
            mesh: Default::default(),
            letsencrypt: Default::default(),
        }
    }
}
