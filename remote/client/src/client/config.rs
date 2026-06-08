//! Configuration for the Terrazzo [Client](super::Client).

use std::ffi::OsString;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::OnceLock;

use reqwest::Url;
use tracing::debug;
use trz_gateway_common::id::ClientName;
use trz_gateway_common::is_global::IsGlobal;
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
}

pub(crate) fn gateway_url<C: ClientConfig>(
    client_config: &C,
    path: &str,
) -> Result<Url, SniOverrideError> {
    let mut url = base_url(client_config)?;
    let path = path.strip_prefix('/').unwrap_or(path);
    url.path_segments_mut()
        .map_err(|()| SniOverrideError::BaseUrlCannotBeABase)?
        .pop_if_empty()
        .extend(path.split('/'));
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

pub(crate) fn base_url<C: ClientConfig>(client_config: &C) -> Result<Url, SniOverrideError> {
    let mut url = Url::parse(&client_config.base_url().to_string())?;
    if let Some(sni_override) = client_config.sni_override() {
        url.set_host(Some(sni_override))
            .map_err(|_| SniOverrideError::InvalidSniOverride(sni_override.to_owned()))?;
    }
    Ok(url)
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

#[derive(thiserror::Error, Debug)]
pub enum SniOverrideError {
    #[error("{0}")]
    Url(#[from] url::ParseError),

    #[error("The Gateway URL cannot be used as a base URL")]
    BaseUrlCannotBeABase,

    #[error("The Gateway URL must include a host when using SNI override")]
    MissingBaseUrlHost,

    #[error("The Gateway URL must include or imply a port when using SNI override")]
    MissingBaseUrlPort,

    #[error("Invalid SNI override: {0}")]
    InvalidSniOverride(String),
}
