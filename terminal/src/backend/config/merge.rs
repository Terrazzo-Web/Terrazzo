use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tracing::warn;
use trz_gateway_common::dynamic_config::has_diff::DiffArc;
use trz_gateway_common::dynamic_config::has_diff::DiffOption;

use super::Config;
use super::ConfigFile;
use super::ConfigImpl;
use super::mesh::MeshConfig;
use super::mesh::MeshWebRtcConfig;
use super::p2p::IceServerConfig;
use super::server::P2pRegistrationConfig;
use super::server::ServerConfig;
use super::types::ConfigFileTypes;
use super::types::RuntimeTypes;
use crate::backend::HOST;
use crate::backend::PORT;
use crate::backend::auth::DEFAULT_TOKEN_LIFETIME;
use crate::backend::auth::DEFAULT_TOKEN_REFRESH;
use crate::backend::cli::Cli;
use crate::backend::home;
use crate::backend::terrazzo_home;

impl ConfigFile {
    pub fn merge(self, cli: &Cli) -> Config {
        Config(ConfigImpl {
            server: merge_server_config(&self.server, cli).into(),
            mesh: merge_mesh_config(self.mesh.as_deref(), cli).into(),
            letsencrypt: self.letsencrypt.clone(),
        })
    }
}

impl Config {
    pub fn to_config_file(&self) -> ConfigFile {
        let ConfigImpl {
            server,
            mesh,
            letsencrypt,
        } = self.deref();
        ConfigFile(ConfigImpl {
            server: DiffArc::from(ServerConfig {
                host: Some(server.host.clone()),
                ports: server.ports.clone(),
                terminal_shell: server.terminal_shell.clone(),
                trash: Some(collapse_tilde(&server.trash)),
                git_trash: server.git_trash.as_ref().map(collapse_tilde),
                tantivy_cache: Some(server.tantivy_cache.clone()),
                search_index_refresh: Some(
                    humantime::format_duration(server.search_index_refresh).to_string(),
                ),
                search_index_stale_after: Some(
                    humantime::format_duration(server.search_index_stale_after).to_string(),
                ),
                set_current_endpoint: server.set_current_endpoint.clone(),
                pidfile: Some(collapse_tilde(&server.pidfile)),
                private_root_ca: Some(collapse_tilde(&server.private_root_ca)),
                password: server.password.clone(),
                token_lifetime: Some(humantime::format_duration(server.token_lifetime).to_string()),
                token_refresh: Some(humantime::format_duration(server.token_refresh).to_string()),
                config_file_watcher: Some(server.config_file_watcher),
                config_file_poll_strategy: server.config_file_poll_strategy.clone(),
                certificate_renewal_threshold: Some(
                    humantime::format_duration(server.certificate_renewal_threshold).to_string(),
                ),
                p2p_registration: server.p2p_registration.as_ref().map(|p2p| {
                    P2pRegistrationConfig {
                        signaling_url: Some(p2p.signaling_url.clone()),
                        server_name: Some(p2p.server_name.clone()),
                        ice_servers: p2p.ice_servers.clone(),
                        retry_strategy: Some(p2p.retry_strategy.clone()),
                        handshake_timeout: Some(
                            humantime::format_duration(p2p.handshake_timeout).to_string(),
                        ),
                        authorization_bearer_token: p2p.authorization_bearer_token.clone(),
                        max_sessions: p2p.max_sessions,
                    }
                }),
            }),
            mesh: DiffOption::from(mesh.as_ref().map(|mesh| {
                DiffArc::from(MeshConfig {
                    client_name: Some(mesh.client_name.clone()),
                    gateway_url: Some(mesh.gateway_url.clone()),
                    sni_override: mesh.sni_override.clone(),
                    gateway_pki: mesh.gateway_pki.as_ref().map(collapse_tilde),
                    client_certificate: Some(collapse_tilde(&mesh.client_certificate)),
                    retry_strategy: Some(mesh.retry_strategy.clone()),
                    client_certificate_renewal: Some(
                        humantime::format_duration(mesh.client_certificate_renewal).to_string(),
                    ),
                    web_rtc: mesh.web_rtc.as_ref().map(|web_rtc| MeshWebRtcConfig {
                        signaling_url: Some(web_rtc.signaling_url.clone()),
                        server_name: Some(web_rtc.server_name.clone()),
                        ice_servers: web_rtc.ice_servers.clone(),
                        signaling_timeout: Some(
                            humantime::format_duration(web_rtc.signaling_timeout).to_string(),
                        ),
                        handshake_timeout: Some(
                            humantime::format_duration(web_rtc.handshake_timeout).to_string(),
                        ),
                        connect_timeout: Some(
                            humantime::format_duration(web_rtc.connect_timeout).to_string(),
                        ),
                    }),
                })
            })),
            letsencrypt: letsencrypt.clone(),
        })
    }
}

fn merge_server_config(
    server: &ServerConfig<ConfigFileTypes>,
    cli: &Cli,
) -> ServerConfig<RuntimeTypes> {
    let port = cli.port.or(server.ports.first().cloned()).unwrap_or(PORT);
    let ports = if !cli.ports.is_empty() {
        std::iter::once(port).chain(cli.ports.clone()).collect()
    } else if !server.ports.is_empty() {
        server.ports.clone()
    } else {
        vec![PORT]
    };
    ServerConfig {
        host: {
            let host = cli.host.as_deref();
            let host = host.or(server.host.as_deref());
            host.unwrap_or(HOST).to_owned()
        },
        ports,
        terminal_shell: cli
            .terminal_shell
            .clone()
            .or_else(|| server.terminal_shell.clone()),
        trash: {
            let trash = cli.trash.as_deref();
            let trash = trash.or(server.trash.as_deref()).map(expand_tilde);
            trash.unwrap_or_else(|| terrazzo_home().join("trash"))
        }
        .into(),
        git_trash: {
            let git_trash = cli.git_trash.as_deref();
            git_trash.or(server.git_trash.as_deref()).map(expand_tilde)
        }
        .map(Arc::from),
        tantivy_cache: cli
            .tantivy_cache
            .as_deref()
            .map(Arc::from)
            .or_else(|| server.tantivy_cache.clone())
            .unwrap_or_else(|| Path::new(".tantivy-cache").into()),
        search_index_refresh: parse_duration(server.search_index_refresh.as_deref())
            .unwrap_or(Duration::from_secs(60 * 60)),
        search_index_stale_after: parse_duration(server.search_index_stale_after.as_deref())
            .unwrap_or(Duration::from_secs(5 * 60)),
        set_current_endpoint: cli.set_current_endpoint.as_deref().map(Arc::from),
        pidfile: {
            let pidfile = cli.pidfile.as_deref();
            let pidfile = pidfile.or(server.pidfile.as_deref()).map(expand_tilde);
            pidfile.unwrap_or_else(|| terrazzo_home().join(format!("terminal-{port}.pid")))
        }
        .into(),
        private_root_ca: {
            let private_root_ca = cli.private_root_ca.as_deref();
            let private_root_ca = private_root_ca
                .or(server.private_root_ca.as_deref())
                .map(expand_tilde);
            private_root_ca.unwrap_or_else(|| terrazzo_home().join("root_ca"))
        }
        .into(),
        password: server.password.clone(),
        token_lifetime: parse_duration(server.token_lifetime.as_deref())
            .unwrap_or(DEFAULT_TOKEN_LIFETIME),
        token_refresh: parse_duration(server.token_refresh.as_deref())
            .unwrap_or(DEFAULT_TOKEN_REFRESH),
        config_file_watcher: server.config_file_watcher.unwrap_or(true),
        config_file_poll_strategy: server.config_file_poll_strategy.clone(),
        certificate_renewal_threshold: parse_duration(
            server.certificate_renewal_threshold.as_deref(),
        )
        .unwrap_or(Duration::from_secs(1) * 3600 * 24 * 30),
        p2p_registration: merge_p2p_registration(server.p2p_registration.as_ref()),
    }
}

fn merge_p2p_registration(
    p2p: Option<&P2pRegistrationConfig<ConfigFileTypes>>,
) -> Option<P2pRegistrationConfig<RuntimeTypes>> {
    let p2p = p2p?;
    Some(P2pRegistrationConfig {
        signaling_url: p2p.signaling_url.clone()?,
        server_name: p2p.server_name.clone()?,
        ice_servers: default_ice_servers(&p2p.ice_servers),
        retry_strategy: p2p.retry_strategy.clone().unwrap_or_default(),
        handshake_timeout: parse_duration(p2p.handshake_timeout.as_deref())
            .unwrap_or(Duration::from_secs(30)),
        authorization_bearer_token: p2p.authorization_bearer_token.clone(),
        max_sessions: Some(p2p.max_sessions.unwrap_or(64)),
    })
}

fn parse_duration(duration: Option<&str>) -> Option<Duration> {
    duration.and_then(|duration| {
        humantime::parse_duration(duration)
            .inspect_err(|error| warn!("Failed to parse '{duration}': {error}"))
            .ok()
    })
}

fn merge_mesh_config(
    mesh: Option<&MeshConfig<ConfigFileTypes>>,
    cli: &Cli,
) -> Option<DiffArc<MeshConfig<RuntimeTypes>>> {
    let client_name = cli.client_name.as_ref().cloned();
    let gateway_url = cli.gateway_url.as_ref().cloned();
    let sni_override = cli.sni_override.as_ref().cloned();
    let gateway_pki = cli.gateway_pki.as_deref();
    let client_certificate = cli.client_certificate.as_deref();
    Some(DiffArc::from(MeshConfig {
        client_name: client_name.or(mesh.and_then(|m| m.client_name.to_owned()))?,
        gateway_url: gateway_url.or(mesh.and_then(|m| m.gateway_url.to_owned()))?,
        sni_override: sni_override.or_else(|| mesh.and_then(|m| m.sni_override.to_owned())),
        gateway_pki: gateway_pki
            .map(expand_tilde)
            .or_else(|| mesh.and_then(|m| m.gateway_pki.as_deref().map(expand_tilde)))
            .map(Arc::from),
        client_certificate: client_certificate
            .map(expand_tilde)
            .or_else(|| mesh.and_then(|m| m.client_certificate.as_deref().map(expand_tilde)))
            .unwrap_or_else(|| terrazzo_home().join("client_certificate"))
            .into(),
        retry_strategy: mesh
            .and_then(|mesh| mesh.retry_strategy.clone())
            .unwrap_or_default(),
        client_certificate_renewal: mesh
            .and_then(|mesh| parse_duration(mesh.client_certificate_renewal.as_deref()))
            .unwrap_or(Duration::from_secs(3600 * 24 * 30)),
        web_rtc: merge_mesh_web_rtc(mesh.and_then(|mesh| mesh.web_rtc.as_ref())),
    }))
}

fn merge_mesh_web_rtc(
    web_rtc: Option<&MeshWebRtcConfig<ConfigFileTypes>>,
) -> Option<MeshWebRtcConfig<RuntimeTypes>> {
    let web_rtc = web_rtc?;
    Some(MeshWebRtcConfig {
        signaling_url: web_rtc.signaling_url.clone()?,
        server_name: web_rtc.server_name.clone()?,
        ice_servers: default_ice_servers(&web_rtc.ice_servers),
        signaling_timeout: parse_duration(web_rtc.signaling_timeout.as_deref())
            .unwrap_or(Duration::from_secs(10)),
        handshake_timeout: parse_duration(web_rtc.handshake_timeout.as_deref())
            .unwrap_or(Duration::from_secs(30)),
        connect_timeout: parse_duration(web_rtc.connect_timeout.as_deref())
            .unwrap_or(Duration::from_secs(45)),
    })
}

fn default_ice_servers(ice_servers: &[IceServerConfig]) -> Vec<IceServerConfig> {
    match ice_servers.is_empty() {
        true => vec![IceServerConfig::google_stun()],
        false => ice_servers.to_vec(),
    }
}

fn expand_tilde(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if let Ok(stripped) = path.strip_prefix("~") {
        return Path::new(&home()).join(stripped);
    }
    path.to_owned()
}

fn collapse_tilde(path: impl AsRef<Path>) -> Arc<Path> {
    let path = path.as_ref();
    path.strip_prefix(home())
        .map(|p| [Path::new("~"), p].into_iter().collect())
        .unwrap_or_else(|_error: std::path::StripPrefixError| path.to_owned())
        .into()
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use trz_gateway_common::dynamic_config::has_diff::DiffArc;
    use trz_gateway_common::p2p::GOOGLE_STUN;
    use trz_gateway_common::retry_strategy::RetryStrategy;

    use super::Config;
    use super::ConfigFile;
    use super::ConfigImpl;
    use super::ServerConfig;
    use super::parse_duration;
    use crate::backend::config::types::RuntimeTypes;
    use crate::backend::home;
    use crate::backend::terrazzo_home;

    #[test]
    fn expand_tilde() {
        assert!(!home().ends_with("/"));
        assert_eq!(
            home().join("home/path"),
            super::expand_tilde("~//home/path")
        );
        assert_eq!(Path::new("~home/path"), super::expand_tilde("~home/path"));
        assert_eq!(
            Path::new("/~/home/path"),
            super::expand_tilde("/~/home/path")
        );
    }

    #[test]
    fn collapse_tilde() {
        assert_eq!(
            "~/home/path",
            super::collapse_tilde(format!("{}/home/path", home().display()))
                .display()
                .to_string()
        );
        assert_eq!(
            format!(
                "~/{}/home/path",
                home().display().to_string().trim_matches('/')
            ),
            super::collapse_tilde(format!("{home}{home}/home/path", home = home().display()))
                .display()
                .to_string()
        );
    }

    #[test]
    fn merge_defaults_to_watcher_enabled_without_polling() {
        let config = ConfigFile::default().merge(&Default::default());

        assert!(config.server.config_file_watcher);
        assert_eq!(config.server.config_file_poll_strategy, None);
    }

    #[test]
    fn config_file_round_trip_preserves_watcher_and_polling() {
        let config = Config::from(ConfigImpl {
            server: DiffArc::from(ServerConfig::<RuntimeTypes> {
                host: "localhost".into(),
                ports: vec![3000],
                terminal_shell: Some("echo test; exec /bin/bash -i".into()),
                trash: terrazzo_home().join("trash").into(),
                git_trash: Some(Path::new(".trash").into()),
                tantivy_cache: Path::new(".search-cache").into(),
                search_index_refresh: Duration::from_secs(7200),
                search_index_stale_after: Duration::from_secs(600),
                set_current_endpoint: None,
                pidfile: terrazzo_home().join("test.pid").into(),
                private_root_ca: terrazzo_home().join("root_ca").into(),
                password: None,
                token_lifetime: parse_duration(Some("5m")).unwrap(),
                token_refresh: parse_duration(Some("4m 50s")).unwrap(),
                config_file_watcher: false,
                config_file_poll_strategy: Some(RetryStrategy::fixed(Duration::from_secs(60))),
                certificate_renewal_threshold: parse_duration(Some("30days")).unwrap(),
                p2p_registration: None,
            }),
            mesh: Default::default(),
            letsencrypt: Default::default(),
        });

        let round_trip = config.to_config_file().merge(&Default::default());

        assert_eq!(
            round_trip.server.terminal_shell.as_deref(),
            Some("echo test; exec /bin/bash -i")
        );
        assert_eq!(&*round_trip.server.trash, terrazzo_home().join("trash"));
        assert_eq!(
            round_trip.server.git_trash.as_deref(),
            Some(Path::new(".trash"))
        );
        assert_eq!(
            &*round_trip.server.tantivy_cache,
            Path::new(".search-cache")
        );
        assert_eq!(
            round_trip.server.search_index_refresh,
            Duration::from_secs(7200)
        );
        assert_eq!(
            round_trip.server.search_index_stale_after,
            Duration::from_secs(600)
        );
        assert!(!round_trip.server.config_file_watcher);
        assert_eq!(
            round_trip.server.config_file_poll_strategy,
            Some(RetryStrategy::fixed(Duration::from_secs(60)))
        );
    }

    #[test]
    fn p2p_config_defaults_and_round_trips() {
        let config_file: ConfigFile = toml::from_str(
            r#"
[server]
ports = []

[server.p2p_registration]
signaling_url = "http://signal.example:3000"
server_name = "gateway-node"
authorization_bearer_token = "secret"

[mesh]
client_name = "client-node"
gateway_url = "https://gateway.internal"

[mesh.web_rtc]
signaling_url = "http://signal.example:3000"
server_name = "gateway-node"
"#,
        )
        .unwrap();

        let config = config_file.merge(&Default::default());
        let web_rtc = config.mesh.as_ref().unwrap().web_rtc.as_ref().unwrap();
        assert_eq!(web_rtc.signaling_url, "http://signal.example:3000");
        assert_eq!(web_rtc.server_name, "gateway-node");
        assert_eq!(web_rtc.ice_servers[0].urls, vec![GOOGLE_STUN]);
        assert_eq!(web_rtc.signaling_timeout, Duration::from_secs(10));
        assert_eq!(web_rtc.handshake_timeout, Duration::from_secs(30));
        assert_eq!(web_rtc.connect_timeout, Duration::from_secs(45));

        let registration = config.server.p2p_registration.as_ref().unwrap();
        assert_eq!(registration.server_name, "gateway-node");
        assert_eq!(registration.ice_servers[0].urls, vec![GOOGLE_STUN]);
        assert_eq!(registration.max_sessions, Some(64));
        assert!(!format!("{registration:?}").contains("secret"));

        let round_trip = config.to_config_file().merge(&Default::default());
        let round_trip_web_rtc = round_trip.mesh.as_ref().unwrap().web_rtc.as_ref().unwrap();
        assert_eq!(round_trip_web_rtc.signaling_url, web_rtc.signaling_url);
        assert_eq!(round_trip_web_rtc.server_name, web_rtc.server_name);
        assert_eq!(round_trip_web_rtc.ice_servers, web_rtc.ice_servers);
        assert_eq!(
            round_trip
                .server
                .p2p_registration
                .as_ref()
                .unwrap()
                .server_name,
            registration.server_name
        );
    }
}
