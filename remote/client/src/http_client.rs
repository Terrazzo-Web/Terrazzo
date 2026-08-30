use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use reqwest::Certificate;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;

use crate::client::config::ClientConfig;
use crate::client::config::SniOverrideError;
use crate::client::config::sni_override_resolution;

pub(super) fn make_http_client<C>(
    client_config: &C,
) -> Result<reqwest::Client, MakeHttpClientError<<C::GatewayPki as TrustedStoreConfig>::Error>>
where
    C: ClientConfig,
{
    let mut builder = reqwest::Client::builder();
    let roots = client_config
        .gateway_pki()
        .root_certificates()
        .map_err(MakeHttpClientError::RootCertificates)?;
    for root in roots.all_certificates() {
        let root_der = root.to_der().map_err(MakeHttpClientError::RootToDer)?;
        let root_certificate =
            Certificate::from_der(&root_der).map_err(MakeHttpClientError::DerToCertificate)?;
        builder = builder.add_root_certificate(root_certificate);
    }
    if let Some((sni_override, socket_addr)) =
        sni_override_resolution(client_config).map_err(MakeHttpClientError::SniOverride)?
    {
        builder = builder.resolve(&sni_override, socket_addr);
    }
    builder.build().map_err(MakeHttpClientError::Build)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MakeHttpClientError<E: std::error::Error> {
    #[error("[{n}] {0}", n = self.name())]
    SniOverride(#[from] SniOverrideError),

    #[error("[{n}] {0}", n = self.name())]
    RootCertificates(E),

    #[error("[{n}] {0}", n = self.name())]
    RootToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    DerToCertificate(reqwest::Error),

    #[error("[{n}] {0}", n = self.name())]
    Build(reqwest::Error),
}
