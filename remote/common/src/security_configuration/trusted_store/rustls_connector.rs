#![cfg(feature = "client")]

use std::future::Future;

use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::error::ErrorStack;
use tokio_rustls::rustls;

use self::rustls::client::WebPkiServerVerifier;
use self::rustls::pki_types::CertificateDer;
use self::rustls::server::VerifierBuilderError;
use super::TrustedStoreConfig;

pub trait ToRustlsConnector: TrustedStoreConfig {
    fn to_rustls_connector(
        &self,
    ) -> impl Future<Output = Result<rustls::ClientConfig, ToRustlsConnectorError<Self::Error>>>
    {
        to_rustls_connector_impl(self)
    }
}

impl<T: TrustedStoreConfig> ToRustlsConnector for T {}

pub async fn to_rustls_connector_impl<T: TrustedStoreConfig + ?Sized>(
    trusted_store: &T,
) -> Result<rustls::ClientConfig, ToRustlsConnectorError<T::Error>> {
    let mut roots = rustls::RootCertStore::empty();
    let trusted_roots = trusted_store
        .root_certificates()
        .map_err(ToRustlsConnectorError::Certificate)?;
    for trusted_root in trusted_roots.all_certificates() {
        roots
            .add(CertificateDer::from_slice(
                &trusted_root
                    .to_der()
                    .map_err(ToRustlsConnectorError::CertificateToDer)?,
            ))
            .map_err(ToRustlsConnectorError::AddCertificate)?;
    }
    let config = rustls::ClientConfig::builder()
        .with_webpki_verifier(WebPkiServerVerifier::builder(roots.into()).build()?)
        .with_no_client_auth();
    Ok(config)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ToRustlsConnectorError<E: std::error::Error> {
    #[error("[{n}] {0}", n = self.name())]
    Certificate(E),

    #[error("[{n}] {0}", n = self.name())]
    CertificateToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    AddCertificate(rustls::Error),

    #[error("[{n}] {0}", n = self.name())]
    VerifierBuilderError(#[from] VerifierBuilderError),
}
