/* TODO: error should be formatted as

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MyErrorCodes {
    #[error("[{n}] <Some error explanation>: {0}", n = self.name())]
    MyErrorCode(MaybeAnotherError),
*/
#[derive(Debug, thiserror::Error)]
pub(super) enum P2pServerError {
    #[error("Invalid P2P server configuration: {0}")]
    InvalidConfig(String),

    #[error("Invalid signaling URL scheme")]
    InvalidUrlScheme,

    #[error("Signaling URL: {0}")]
    Url(#[from] url::ParseError),

    #[error("Signaling request: {0}")]
    Request(#[from] tokio_tungstenite::tungstenite::http::Error),

    #[error("Signaling authorization header: {0}")]
    Header(#[from] tokio_tungstenite::tungstenite::http::header::InvalidHeaderValue),

    #[error("Signaling WebSocket: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("Signaling JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Signaling protocol: {0}")]
    Validation(#[from] trz_gateway_common::p2p::protocol::ValidationError),

    #[error("Signaling protocol: {0}")]
    Protocol(String),

    #[error("Signaling registration closed")]
    RegistrationClosed,

    #[error("Peer cancelled: {0}")]
    PeerCancelled(String),

    #[error("WebRTC handshake timed out")]
    HandshakeTimeout,

    #[error("Server is shutting down")]
    Shutdown,

    #[error("WebRTC: {0}")]
    PeerConnection(#[from] trz_gateway_common::p2p::peer_connection::PeerConnectionError),

    #[error("WebRTC session task: {0}")]
    SessionTask(tokio::task::JoinError),

    #[error("P2P TLS: {0}")]
    Tls(#[from] std::io::Error),

    #[error("P2P HTTP: {0}")]
    ServeHttp(Box<dyn std::error::Error + Send + Sync>),
}

impl P2pServerError {
    pub(super) fn should_report_to_peer(&self) -> bool {
        !matches!(
            self,
            Self::RegistrationClosed | Self::PeerCancelled(_) | Self::Shutdown
        )
    }

    pub(super) fn peer_detail(&self) -> &'static str {
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
