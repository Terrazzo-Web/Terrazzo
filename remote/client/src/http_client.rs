use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use reqwest::Certificate;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;

pub(super) fn make_http_client<C>(
    gateway_pki: C,
) -> Result<reqwest::Client, MakeHttpClientError<C::Error>>
where
    C: TrustedStoreConfig,
{
    let mut builder = reqwest::Client::builder();
    let roots = gateway_pki
        .root_certificates()
        .map_err(MakeHttpClientError::RootCertificates)?;
    for root in roots.all_certificates() {
        let root_der = root.to_der().map_err(MakeHttpClientError::RootToDer)?;
        let root_certificate =
            Certificate::from_der(&root_der).map_err(MakeHttpClientError::DerToCertificate)?;
        builder = builder.add_root_certificate(root_certificate);
    }
    builder.build().map_err(MakeHttpClientError::Build)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MakeHttpClientError<E: std::error::Error> {
    #[error("[{n}] {0}", n = self.name())]
    RootCertificates(E),

    #[error("[{n}] {0}", n = self.name())]
    RootToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    DerToCertificate(reqwest::Error),

    #[error("[{n}] {0}", n = self.name())]
    Build(reqwest::Error),
}
