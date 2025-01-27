use std::future::Future;
use std::sync::Arc;

use axum_server::tls_rustls::RustlsConfig;
use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::error::ErrorStack;
use openssl::pkey::PKey;
use openssl::pkey::Private;
use openssl::x509::X509Ref;
use openssl::x509::X509;
use tracing::debug;

use super::common::parse_pem_certificates;

pub trait CertificateConfig {
    type Error: std::error::Error;
    fn intermediates(&self) -> Result<Vec<X509>, Self::Error>;
    fn certificate(&self) -> Result<Certificate, Self::Error>;
    fn to_rustls_config(
        &self,
    ) -> impl Future<Output = Result<RustlsConfig, ToRustlsConfigError<Self::Error>>> {
        to_rustls_config_impl(self)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PemCertificate {
    pub intermediates_pem: String,
    pub certificate_pem: String,
    pub private_key_pem: String,
}

impl CertificateConfig for PemCertificate {
    type Error = PemCertificateError;

    fn certificate(&self) -> Result<Certificate, Self::Error> {
        let certificate = X509::from_pem(self.certificate_pem.as_bytes())
            .map_err(PemCertificateError::InvalidPemCertificate)?;
        let private_key = PKey::private_key_from_pem(self.private_key_pem.as_bytes())
            .map_err(PemCertificateError::InvalidPemPrivateKey)?;
        Ok(Certificate {
            certificate,
            private_key,
        })
    }

    fn intermediates(&self) -> Result<Vec<X509>, Self::Error> {
        let mut intermediates = vec![];
        for intermediate in parse_pem_certificates(&self.intermediates_pem) {
            let intermediate = intermediate.map_err(PemCertificateError::InvalidPemCertificate)?;
            intermediates.push(intermediate);
        }
        Ok(intermediates)
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum PemCertificateError {
    #[error("[{n}] Invalid PEM certificate: {0}", n = self.name())]
    InvalidPemCertificate(ErrorStack),

    #[error("[{n}] Invalid X509 certificate: {0}", n = self.name())]
    InvalidPemPrivateKey(ErrorStack),
}

pub struct Certificate {
    pub certificate: X509,
    pub private_key: PKey<Private>,
}

impl Certificate {
    pub fn display(&self) -> impl std::fmt::Display {
        display_x509_certificate(&self.certificate)
    }
}

pub fn display_x509_certificate(certificate: &X509Ref) -> impl std::fmt::Display {
    certificate
        .to_text()
        .map(String::from_utf8)
        .unwrap_or_else(|error| Ok(error.to_string()))
        .unwrap_or_else(|error| error.to_string())
}

async fn to_rustls_config_impl<T: CertificateConfig + ?Sized>(
    certificate_config: &T,
) -> Result<RustlsConfig, ToRustlsConfigError<T::Error>> {
    let certificate = certificate_config
        .certificate()
        .map_err(ToRustlsConfigError::Certificates)?;
    let mut certificate_chain = vec![];

    debug!("Add leaf certificate: {}", certificate.display());
    certificate_chain.push(
        certificate
            .certificate
            .to_der()
            .map_err(ToRustlsConfigError::CertificateToDer)?,
    );

    for intermediate in certificate_config
        .intermediates()
        .map_err(ToRustlsConfigError::Intermediates)?
    {
        debug!(
            "Add intermediate: {}",
            display_x509_certificate(&intermediate)
        );
        let intermediate = intermediate.to_der();
        certificate_chain.push(intermediate.map_err(ToRustlsConfigError::IntermediateToDer)?);
    }

    let private_key = certificate
        .private_key
        .private_key_to_der()
        .map_err(ToRustlsConfigError::PrivateKeyToDer)?;
    RustlsConfig::from_der(certificate_chain, private_key)
        .await
        .map_err(ToRustlsConfigError::RustlsConfig)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ToRustlsConfigError<E: std::error::Error> {
    #[error("[{n}] {0}", n = self.name())]
    Certificates(E),

    #[error("[{n}] {0}", n = self.name())]
    Intermediates(E),

    #[error("[{n}] {0}", n = self.name())]
    IntermediateToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    CertificateToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    PrivateKeyToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    RustlsConfig(std::io::Error),
}

impl<T: CertificateConfig> CertificateConfig for Arc<T> {
    type Error = T::Error;

    fn intermediates(&self) -> Result<Vec<X509>, Self::Error> {
        self.as_ref().intermediates()
    }

    fn certificate(&self) -> Result<Certificate, Self::Error> {
        self.as_ref().certificate()
    }

    fn to_rustls_config(
        &self,
    ) -> impl Future<Output = Result<RustlsConfig, ToRustlsConfigError<Self::Error>>> {
        self.as_ref().to_rustls_config()
    }
}

fn to() {
    fn sfd() {
        let mut roots = RootCertStore::empty();
        roots
            .add(self.root_ca.certificate.to_der().unwrap().into())
            .unwrap();
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(
                WebPkiServerVerifier::builder(roots.into()).build().unwrap(),
            )
            .with_no_client_auth();
        let config = TlsConnector::from(Arc::new(config));
    }
}
