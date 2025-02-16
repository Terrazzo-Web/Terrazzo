use std::sync::Arc;

use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::error::ErrorStack;
use openssl::pkey::PKey;
use openssl::x509::store::X509Store;
use openssl::x509::store::X509StoreBuilder;
use openssl::x509::X509;

use super::Certificate;
use super::CertificateConfig;
use crate::certificate_info::CertificateInfo;
use crate::security_configuration::common::parse_pem_certificates;
use crate::security_configuration::trusted_store::TrustedStoreConfig;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PemCertificate {
    pub intermediates_pem: String,
    pub certificate_pem: String,
    pub private_key_pem: String,
}

impl CertificateConfig for PemCertificate {
    type Error = PemCertificateError;

    fn certificate(&self) -> Result<Arc<Certificate>, Self::Error> {
        let certificate = X509::from_pem(self.certificate_pem.as_bytes())
            .map_err(PemCertificateError::InvalidPemCertificate)?;
        let private_key = PKey::private_key_from_pem(self.private_key_pem.as_bytes())
            .map_err(PemCertificateError::InvalidPemPrivateKey)?;
        Ok(Certificate {
            certificate,
            private_key,
        }
        .into())
    }

    fn intermediates(&self) -> Result<Arc<Vec<X509>>, Self::Error> {
        let mut intermediates = vec![];
        for intermediate in parse_pem_certificates(&self.intermediates_pem) {
            let intermediate = intermediate.map_err(PemCertificateError::InvalidPemCertificate)?;
            intermediates.push(intermediate);
        }
        Ok(intermediates.into())
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

impl TrustedStoreConfig for PemCertificate {
    type Error = PemTrustedStoreCertificateError;
    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        let mut trusted_roots =
            X509StoreBuilder::new().map_err(PemTrustedStoreCertificateError::X509StoreBuilder)?;
        trusted_roots
            .add_cert(self.certificate()?.certificate.clone())
            .map_err(PemTrustedStoreCertificateError::AddCert)?;
        Ok(Arc::new(trusted_roots.build()))
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum PemTrustedStoreCertificateError {
    #[error("[{n}] Failed to create X509StoreBuilder: {0}", n = self.name())]
    X509StoreBuilder(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    Certificate(#[from] PemCertificateError),

    #[error("[{n}] Failed to add a X509 certificate to the store: {0}", n = self.name())]
    AddCert(ErrorStack),
}

impl From<CertificateInfo<String>> for PemCertificate {
    fn from(value: CertificateInfo<String>) -> Self {
        Self {
            intermediates_pem: String::default(),
            certificate_pem: value.certificate,
            private_key_pem: value.private_key,
        }
    }
}
