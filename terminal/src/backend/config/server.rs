use std::ops::Deref;
use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;
use trz_gateway_common::dynamic_config::DynamicConfig;
use trz_gateway_common::dynamic_config::has_diff::DiffArc;
use trz_gateway_common::p2p::credential::Credential;

use super::types::ConfigTypes;
use super::types::Password;
use super::types::RuntimeTypes;
use crate::backend::config::p2p::IceServerConfig;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ServerConfig<T: ConfigTypes = RuntimeTypes> {
    /// The TCP host to listen to.
    pub host: T::String,

    /// The TCP ports to listen to.
    pub ports: T::Ports,

    /// The shell command to run for new terminals.
    #[serde(rename = "terminal-shell", alias = "terminal_shell")]
    pub terminal_shell: Option<String>,

    /// The folder where deleted text-editor files are moved.
    pub trash: T::Path,

    /// The folder, relative to a Git repository root, where deleted Git files are moved.
    pub git_trash: T::MaybePath,

    /// The folder, relative to a Git repository root, where Tantivy indexes are stored.
    pub tantivy_cache: T::Path,

    /// How often to fully reconcile cached search indexes.
    pub search_index_refresh: T::Duration,

    /// How old a full reconciliation may be when a search finishes.
    pub search_index_stale_after: T::Duration,

    pub set_current_endpoint: T::MaybePath,

    /// The file to store the pid of the daemon while it is running,
    pub pidfile: T::Path,

    /// The file to the store private Root CA.
    pub private_root_ca: T::Path,

    /// The password to login to the UI.
    pub password: Option<Password>,
    pub token_lifetime: T::Duration,
    pub token_refresh: T::Duration,

    /// Whether to watch the config file for live updates.
    pub config_file_watcher: T::MaybeBool,

    /// Polling strategy for the config file
    pub config_file_poll_strategy: T::MaybeRetryStrategy,

    /// Certificates renewal strategy
    pub certificate_renewal_threshold: T::Duration,

    /// Optional outbound WebRTC registration for serving this gateway behind NAT.
    pub p2p_registration: Option<P2pRegistrationConfig<T>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct P2pRegistrationConfig<T: ConfigTypes = RuntimeTypes> {
    pub signaling_url: T::String,
    pub server_name: T::String,

    #[serde(default)]
    pub ice_servers: Vec<IceServerConfig>,

    pub retry_strategy: T::RetryStrategy,
    pub handshake_timeout: T::Duration,
    pub authorization_bearer_token: Option<Credential>,
    pub max_sessions: Option<usize>,
}

#[derive(Clone)]
pub struct DynamicServerConfig(pub(super) Arc<DynamicConfig<DiffArc<ServerConfig>>>);

impl Deref for DynamicServerConfig {
    type Target = Arc<DynamicConfig<DiffArc<ServerConfig>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Arc<DynamicConfig<DiffArc<ServerConfig>>>> for DynamicServerConfig {
    fn from(value: Arc<DynamicConfig<DiffArc<ServerConfig>>>) -> Self {
        Self(value)
    }
}
