//! Configuration for the Terrazzo [Client](super::Client).

use std::ffi::OsString;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use reqwest::Url;
use tracing::debug;
use trz_gateway_common::id::ClientName;
use trz_gateway_common::is_global::IsGlobal;
use trz_gateway_common::p2p::GOOGLE_STUN;
use trz_gateway_common::p2p::peer_connection::IceServer;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;
use uuid::Uuid;

/// Configuration for the Terrazzo client.
///
/// This is used to:
/// 1. Securely fetch and cache the client certificate,
/// 2. Securely connect to the Terrazzo Gateway.
///
/// Both cases require the base URL to connect to and the PKI to trust.
///
/// Used by [TunnelConfig] to create tunnels using the certificate obtained from [ClientConfig]
///
/// [TunnelConfig]: crate::tunnel_config::TunnelConfig
pub trait ClientConfig: IsGlobal {
    /// The URL where the Terrazzo Gateway is listening.
    fn base_url(&self) -> impl std::fmt::Display {
        let port = if cfg!(debug_assertions) { 3000 } else { 3001 };
        format!("https://localhost:{port}")
    }

    /// A unique name for the client.
    ///
    /// Defaults to the hostname.
    fn client_name(&self) -> ClientName {
        static CLIENT_ID: OnceLock<ClientName> = OnceLock::new();
        fn make_default_hostname() -> ClientName {
            match hostname::get().map(OsString::into_string) {
                Ok(Ok(hostname)) => return hostname.into(),
                Err(error) => debug!("Failed to get the hostname with hostname::get(): {error}"),
                Ok(Err(error)) => debug!("Failed to parse the hostname string: {error:?}"),
            }
            return Uuid::new_v4().to_string().into();
        }

        CLIENT_ID.get_or_init(make_default_hostname).clone()
    }

    /// The PKI to trust when connecting to the Terrazzo Gateway.
    type GatewayPki: TrustedStoreConfig;

    /// The trust anchors for the Terrazzo Gateway server certificate.
    fn gateway_pki(&self) -> Self::GatewayPki;

    /// The TLS server name to validate, when it differs from [ClientConfig::base_url].
    ///
    /// This is useful when connecting to an IP address while validating the
    /// certificate against a DNS name.
    fn sni_override(&self) -> Option<&str> {
        None
    }

    /// Selects how the Gateway is reached. Direct sockets remain the default.
    fn transport(&self) -> ClientTransport {
        ClientTransport::Direct
    }
}

impl<T: ClientConfig> ClientConfig for Arc<T> {
    fn base_url(&self) -> impl std::fmt::Display {
        self.as_ref().base_url()
    }

    fn client_name(&self) -> ClientName {
        self.as_ref().client_name()
    }

    type GatewayPki = T::GatewayPki;
    fn gateway_pki(&self) -> Self::GatewayPki {
        self.as_ref().gateway_pki()
    }

    fn sni_override(&self) -> Option<&str> {
        self.as_ref().sni_override()
    }

    fn transport(&self) -> ClientTransport {
        self.as_ref().transport()
    }
}

/// Network transport used to reach the Gateway.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ClientTransport {
    /// Connect directly to the host and port in [`ClientConfig::base_url`].
    #[default]
    Direct,

    /// Reach a NATed Gateway through its public signaling server.
    WebRtc(P2pClientConfig),
}

/// Client-side WebRTC signaling and ICE configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P2pClientConfig {
    /// Public signaling server base URL.
    pub signaling_url: String,

    /// Globally unique name registered by the target Gateway.
    pub server_name: ClientName,

    /// STUN or TURN servers used to discover a viable ICE pair.
    pub ice_servers: Vec<IceServer>,

    /// Maximum time to establish the signaling WebSocket and receive a session ID.
    pub signaling_timeout: Duration,

    /// Maximum time to exchange SDP/ICE and open the reliable data channel.
    pub handshake_timeout: Duration,

    /// Overall deadline for the complete P2P connection attempt.
    pub connect_timeout: Duration,
}

impl P2pClientConfig {
    /// Creates a configuration with bounded timeouts and Google's public STUN server.
    pub fn new(signaling_url: impl Into<String>, server_name: ClientName) -> Self {
        Self {
            signaling_url: signaling_url.into(),
            server_name,
            ice_servers: vec![IceServer {
                urls: vec![GOOGLE_STUN.to_owned()],
                ..IceServer::default()
            }],
            signaling_timeout: Duration::from_secs(10),
            handshake_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(45),
        }
    }
}

pub(crate) fn url<C: ClientConfig>(client_config: &C, path: &str) -> Result<Url, SniOverrideError> {
    Ok(Url::parse(&format!("{}{path}", client_config.base_url()))?)
}

pub(crate) fn set_sni_override(
    url: &mut Url,
    sni_override: Option<&str>,
) -> Result<(), SniOverrideError> {
    if let Some(sni_override) = sni_override {
        url.set_host(Some(sni_override))
            .map_err(|_| SniOverrideError::InvalidSniOverride(sni_override.to_owned()))?;
    }
    Ok(())
}

pub(crate) fn sni_override_resolution<C: ClientConfig>(
    client_config: &C,
) -> Result<Option<(String, SocketAddr)>, SniOverrideError> {
    let Some(sni_override) = client_config.sni_override() else {
        return Ok(None);
    };
    let url = Url::parse(&client_config.base_url().to_string())?;
    let Some(host) = url.host_str() else {
        return Err(SniOverrideError::MissingBaseUrlHost);
    };
    let Ok(ip) = host.parse::<IpAddr>() else {
        return Ok(None);
    };
    let port = url
        .port_or_known_default()
        .ok_or(SniOverrideError::MissingBaseUrlPort)?;
    Ok(Some((sni_override.to_owned(), SocketAddr::new(ip, port))))
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum SniOverrideError {
    #[error("[{n}] Failed to parse the Gateway URL: {0}", n = self.name())]
    Url(#[from] url::ParseError),

    #[error("[{n}] The Gateway URL must include a host when using SNI override", n = self.name())]
    MissingBaseUrlHost,

    #[error("[{n}] The Gateway URL must include or imply a port when using SNI override", n = self.name())]
    MissingBaseUrlPort,

    #[error("[{n}] Invalid SNI override: {0}", n = self.name())]
    InvalidSniOverride(String),
}

#[cfg(test)]
mod tests {
    use trz_gateway_common::p2p::GOOGLE_STUN;

    use super::*;

    #[test]
    fn transport_defaults_to_direct_and_p2p_defaults_are_bounded() {
        assert_eq!(ClientTransport::Direct, ClientTransport::default());

        let config = P2pClientConfig::new("https://signal.example", "gateway".into());
        assert_eq!("https://signal.example", config.signaling_url);
        assert_eq!("gateway", config.server_name.as_ref());
        assert_eq!(vec![GOOGLE_STUN], config.ice_servers[0].urls);
        assert!(config.signaling_timeout < config.handshake_timeout);
        assert!(config.handshake_timeout < config.connect_timeout);
    }
}
