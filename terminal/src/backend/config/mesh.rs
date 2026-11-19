use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;
use trz_gateway_common::certificate_info::CertificateInfo;

use super::types::ConfigTypes;
use super::types::RuntimeTypes;
use crate::backend::config::p2p::IceServerConfig;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MeshConfig<T: ConfigTypes = RuntimeTypes> {
    /// The Client name.
    pub client_name: T::String,

    /// The Gateway endpoint.
    pub gateway_url: T::String,

    /// The TLS server name to validate when connecting to the Gateway endpoint.
    pub gateway_sni_override: T::MaybeString,

    /// The Gateway CA.
    ///
    /// This is the Root CA of the Gateway server certificate.
    pub gateway_pki: T::MaybePath,

    /// The file to store the client certificate.
    pub client_certificate: T::Path,

    /// The strategy to retry connecting.
    pub retry_strategy: T::RetryStrategy,

    pub client_certificate_renewal: T::Duration,

    /// Optional WebRTC transport. When absent, connect directly to `gateway_url`.
    pub web_rtc: Option<MeshWebRtcConfig<T>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MeshWebRtcConfig<T: ConfigTypes = RuntimeTypes> {
    /// Public signaling node URL.
    pub signaling_url: T::String,

    /// Routing name registered by the target gateway.
    pub server_name: T::String,

    /// STUN/TURN servers. An empty list defaults to GOOGLE_STUN during merge.
    #[serde(default)]
    pub ice_servers: Vec<IceServerConfig>,

    pub signaling_timeout: T::Duration,
    pub handshake_timeout: T::Duration,
    pub connect_timeout: T::Duration,
}

impl MeshConfig {
    pub fn client_certificate_paths(&self) -> CertificateInfo<PathBuf> {
        const CLIENT_CERTIFICATE_FILE_SUFFIX: CertificateInfo<&str> = CertificateInfo {
            certificate: "cert",
            private_key: "key",
        };

        CLIENT_CERTIFICATE_FILE_SUFFIX
            .map(|suffix| self.client_certificate.with_added_extension(suffix))
    }
}
