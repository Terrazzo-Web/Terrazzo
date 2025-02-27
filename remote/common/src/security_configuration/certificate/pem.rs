use std::sync::Arc;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use openssl::pkey::PKey;
use openssl::x509::X509;

use super::CertificateConfig;
use super::X509CertificateInfo;
use crate::certificate_info::CertificateInfo;
use crate::security_configuration::common::parse_pem_certificates;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PemCertificate {
    pub intermediates_pem: String,
    pub certificate_pem: String,
    pub private_key_pem: String,
}

impl CertificateConfig for PemCertificate {
    type Error = PemCertificateError;

    fn certificate(&self) -> Result<Arc<X509CertificateInfo>, Self::Error> {
        let certificate = X509::from_pem(self.certificate_pem.as_bytes())
            .map_err(PemCertificateError::InvalidPemCertificate)?;
        let private_key = PKey::private_key_from_pem(self.private_key_pem.as_bytes())
            .map_err(PemCertificateError::InvalidPemPrivateKey)?;
        Ok(X509CertificateInfo {
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

impl From<CertificateInfo<String>> for PemCertificate {
    fn from(value: CertificateInfo<String>) -> Self {
        Self {
            intermediates_pem: String::default(),
            certificate_pem: value.certificate,
            private_key_pem: value.private_key,
        }
    }
}
