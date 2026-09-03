use serde::Deserialize;
use serde::Serialize;
use trz_gateway_common::p2p::GOOGLE_STUN;
use trz_gateway_common::p2p::peer_connection::IceServer;

/// Serializable STUN/TURN configuration used by terminal configuration files.
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct IceServerConfig {
    pub urls: Vec<String>,

    #[serde(default)]
    pub username: String,

    #[serde(default)]
    pub credential: String,
}

impl IceServerConfig {
    pub fn google_stun() -> Self {
        Self {
            urls: vec![GOOGLE_STUN.to_owned()],
            ..Self::default()
        }
    }
}

impl std::fmt::Debug for IceServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IceServerConfig")
            .field("urls", &self.urls)
            .field("username", &self.username)
            .field(
                "credential",
                &(!self.credential.is_empty()).then_some("[REDACTED]"),
            )
            .finish()
    }
}

impl From<IceServerConfig> for IceServer {
    fn from(value: IceServerConfig) -> Self {
        Self {
            urls: value.urls,
            username: value.username,
            credential: value.credential,
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct RedactedString(pub String);

impl std::fmt::Debug for RedactedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}
