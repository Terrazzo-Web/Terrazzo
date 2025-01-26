use std::sync::Arc;

use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::error::ErrorStack;
use openssl::pkey::PKey;
use openssl::pkey::Private;
use openssl::x509::X509;

use crate::utils::is_configuration::IsConfiguration;

pub trait SecurityConfig: IsConfiguration {
    fn certificate_pem(&self) -> &str;
    fn private_key_pem(&self) -> &str;

    fn certificate(&self) -> Result<Certificate, CertificateError> {
        let certificate = X509::from_pem(self.certificate_pem().as_bytes())
            .map_err(CertificateError::InvalidPemCertificate)?;
        let private_key = PKey::private_key_from_pem(self.private_key_pem().as_bytes())
            .map_err(CertificateError::InvalidPemPrivateKey)?;
        Ok(Certificate {
            certificate,
            private_key,
        })
    }
}

pub struct Certificate {
    pub certificate: X509,
    pub private_key: PKey<Private>,
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum CertificateError {
    #[error("[{n}] Invalid PEM certificate: {0}", n = self.name())]
    InvalidPemCertificate(ErrorStack),

    #[error("[{n}] Invalid X509 certificate: {0}", n = self.name())]
    InvalidPemPrivateKey(ErrorStack),
}

impl<T: SecurityConfig> SecurityConfig for Arc<T> {
    fn certificate_pem(&self) -> &str {
        let this: &T = self.as_ref();
        this.certificate_pem()
    }

    fn private_key_pem(&self) -> &str {
        let this: &T = self.as_ref();
        this.private_key_pem()
    }

    fn certificate(&self) -> Result<Certificate, CertificateError> {
        let this: &T = self.as_ref();
        this.certificate()
    }
}
