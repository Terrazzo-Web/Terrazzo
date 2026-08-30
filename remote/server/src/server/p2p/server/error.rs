use nameth::NamedEnumValues as _;
use nameth::nameth;

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum P2pServerError {
    #[error("[{n}] Invalid P2P server configuration: {0}", n = self.name())]
    InvalidConfig(String),

    #[error("[{n}] Invalid signaling URL scheme", n = self.name())]
    InvalidUrlScheme,

    #[error("[{n}] Invalid signaling URL: {0}", n = self.name())]
    Url(#[from] url::ParseError),

    #[error("[{n}] Failed to build signaling request: {0}", n = self.name())]
    Request(#[from] tokio_tungstenite::tungstenite::http::Error),

    #[error("[{n}] Invalid signaling authorization header: {0}", n = self.name())]
    Header(#[from] tokio_tungstenite::tungstenite::http::header::InvalidHeaderValue),

    #[error("[{n}] Signaling WebSocket failed: {0}", n = self.name())]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("[{n}] Invalid signaling JSON: {0}", n = self.name())]
    Json(#[from] serde_json::Error),

    #[error("[{n}] Invalid signaling protocol message: {0}", n = self.name())]
    Validation(#[from] trz_gateway_common::p2p::protocol::ValidationError),

    #[error("[{n}] Signaling protocol error: {0}", n = self.name())]
    Protocol(String),

    #[error("[{n}] Signaling registration closed", n = self.name())]
    RegistrationClosed,

    #[error("[{n}] Peer cancelled the session: {0}", n = self.name())]
    PeerCancelled(String),

    #[error("[{n}] WebRTC handshake timed out", n = self.name())]
    HandshakeTimeout,

    #[error("[{n}] Server is shutting down", n = self.name())]
    Shutdown,

    #[error("[{n}] WebRTC connection failed: {0}", n = self.name())]
    PeerConnection(#[from] trz_gateway_common::p2p::peer_connection::PeerConnectionError),

    #[error("[{n}] WebRTC session task failed: {0}", n = self.name())]
    SessionTask(tokio::task::JoinError),

    #[error("[{n}] P2P TLS connection failed: {0}", n = self.name())]
    Tls(#[from] std::io::Error),

    #[error("[{n}] P2P HTTP connection failed: {0}", n = self.name())]
    ServeHttp(Box<dyn std::error::Error + Send + Sync>),
}

impl P2pServerError {
    pub fn should_report_to_peer(&self) -> bool {
        !matches!(
            self,
            Self::RegistrationClosed | Self::PeerCancelled(_) | Self::Shutdown
        )
    }

    pub fn peer_detail(&self) -> &'static str {
        match self {
            Self::HandshakeTimeout => {
                "WebRTC handshake timed out; configure TURN when direct ICE is unavailable"
            }
            Self::Protocol(_) | Self::Validation(_) => "Invalid P2P signaling message",
            Self::PeerConnection(_) | Self::SessionTask(_) => {
                "WebRTC negotiation failed; check STUN/TURN configuration"
            }
            _ => "P2P session failed",
        }
    }
}
