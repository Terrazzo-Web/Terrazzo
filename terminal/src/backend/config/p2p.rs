use serde::Deserialize;
use serde::Serialize;
use trz_gateway_common::p2p::GOOGLE_STUN;
use trz_gateway_common::p2p::credential::Credential;
use trz_gateway_common::p2p::peer_connection::IceServer;

/// Serializable STUN/TURN configuration used by terminal configuration files.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct IceServerConfig {
    pub urls: Vec<String>,

    #[serde(default)]
    pub username: String,

    #[serde(default)]
    pub credential: Credential,
}

impl IceServerConfig {
    pub fn google_stun() -> Self {
        Self {
            urls: vec![GOOGLE_STUN.to_owned()],
            ..Self::default()
        }
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
