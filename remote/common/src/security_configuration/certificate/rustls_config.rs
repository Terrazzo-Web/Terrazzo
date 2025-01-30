#![cfg(feature = "server")]

use std::future::Future;

use axum_server::tls_rustls::RustlsConfig;
use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::error::ErrorStack;
use tracing::debug;

use super::CertificateConfig;
use crate::security_configuration::certificate::display_x509_certificate;

pub trait ToRustlsConfig: CertificateConfig {
    fn to_rustls_config(
        &self,
    ) -> impl Future<Output = Result<RustlsConfig, ToRustlsConfigError<Self::Error>>> {
        to_rustls_config_impl(self)
    }
}

impl<T: CertificateConfig> ToRustlsConfig for T {}

async fn to_rustls_config_impl<T: CertificateConfig + ?Sized>(
    certificate_config: &T,
) -> Result<RustlsConfig, ToRustlsConfigError<T::Error>> {
    let certificate = certificate_config
        .certificate()
        .map_err(ToRustlsConfigError::Certificate)?;
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
        .iter()
    {
        debug!(
            "Add intermediate: {}",
            display_x509_certificate(intermediate)
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
    Certificate(E),

    #[error("[{n}] {0}", n = self.name())]
    CertificateToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    Intermediates(E),

    #[error("[{n}] {0}", n = self.name())]
    IntermediateToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    PrivateKeyToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    RustlsConfig(std::io::Error),
}
