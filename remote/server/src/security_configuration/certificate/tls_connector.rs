use std::future::Future;
use std::sync::Arc;

use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::error::ErrorStack;
use tokio_rustls::rustls;
use tokio_rustls::TlsConnector;

use self::rustls::client::WebPkiServerVerifier;
use self::rustls::server::VerifierBuilderError;
use self::rustls::ClientConfig;
use self::rustls::RootCertStore;
use super::CertificateConfig;

pub trait ToTlsConnector: CertificateConfig {
    fn to_tls_connector(
        &self,
    ) -> impl Future<Output = Result<TlsConnector, ToTlsConnectorError<Self::Error>>> {
        to_tls_connector_impl(self)
    }
}

impl<T: CertificateConfig> ToTlsConnector for T {}

async fn to_tls_connector_impl<T: CertificateConfig + ?Sized>(
    certificate_config: &T,
) -> Result<TlsConnector, ToTlsConnectorError<T::Error>> {
    let mut roots = RootCertStore::empty();
    let certificate = certificate_config
        .certificate()
        .map_err(ToTlsConnectorError::Certificate)?
        .certificate
        .to_der()
        .map_err(ToTlsConnectorError::CertificateToDer)?
        .into();

    roots
        .add(certificate)
        .map_err(ToTlsConnectorError::AddCertificate)?;
    let config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(
            WebPkiServerVerifier::builder(roots.into()).build().unwrap(),
        )
        .with_no_client_auth();
    Ok(TlsConnector::from(Arc::new(config)))
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ToTlsConnectorError<E: std::error::Error> {
    #[error("[{n}] {0}", n = self.name())]
    Certificate(E),

    #[error("[{n}] {0}", n = self.name())]
    CertificateToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    AddCertificate(rustls::Error),

    #[error("[{n}] {0}", n = self.name())]
    VerifierBuilderError(#[from] VerifierBuilderError),
}
