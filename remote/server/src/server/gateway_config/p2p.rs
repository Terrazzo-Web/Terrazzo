//! Configuration for registering a NATed gateway with a signaling node.

use std::time::Duration;

use trz_gateway_common::id::ClientName;
use trz_gateway_common::p2p::GOOGLE_STUN;
use trz_gateway_common::p2p::peer_connection::IceServer;
use trz_gateway_common::retry_strategy::RetryStrategy;

/// Outbound registration settings for the WebRTC server role.
#[derive(Clone, Debug)]
pub struct P2pRegistrationConfig {
    /// Public signaling-node URL. HTTP(S) schemes are converted to WS(S).
    pub signaling_url: String,

    /// Globally unique routing name registered with the signaling node.
    pub server_name: ClientName,

    /// STUN and TURN servers supplied to each answering peer connection.
    pub ice_servers: Vec<IceServer>,

    /// Delay strategy used after the persistent registration disconnects.
    pub retry_strategy: RetryStrategy,

    /// Maximum time allowed for one WebRTC negotiation.
    pub handshake_timeout: Duration,

    /// Optional registration credential sent to the signaling node.
    pub authorization: Option<P2pRegistrationAuthorization>,

    /// Maximum concurrent negotiations and active P2P HTTP connections.
    pub max_sessions: usize,
}

impl P2pRegistrationConfig {
    /// Creates settings with Google STUN, bounded exponential retry, and 64 sessions.
    pub fn new(signaling_url: impl Into<String>, server_name: ClientName) -> Self {
        Self {
            signaling_url: signaling_url.into(),
            server_name,
            ice_servers: vec![IceServer {
                urls: vec![GOOGLE_STUN.into()],
                ..IceServer::default()
            }],
            retry_strategy: RetryStrategy::default(),
            handshake_timeout: Duration::from_secs(30),
            authorization: None,
            max_sessions: 64,
        }
    }
}

/// Credential attached to an outbound signaling registration request.
#[derive(Clone)]
pub enum P2pRegistrationAuthorization {
    /// HTTP `Authorization: Bearer ...` credential.
    BearerToken(String),
}

impl std::fmt::Debug for P2pRegistrationAuthorization {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BearerToken(_) => formatter.write_str("BearerToken([REDACTED])"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_bounded_and_credentials_are_redacted() {
        let mut config = P2pRegistrationConfig::new("https://signal.example", "server".into());
        config.authorization = Some(P2pRegistrationAuthorization::BearerToken("secret".into()));
        assert_eq!(64, config.max_sessions);
        assert_eq!(Duration::from_secs(30), config.handshake_timeout);
        assert!(config.ice_servers[0].urls[0].contains("stun.l.google.com"));
        let debug = format!("{config:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("secret"));
    }
}
