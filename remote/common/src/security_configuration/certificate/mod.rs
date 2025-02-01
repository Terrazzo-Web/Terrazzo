use std::sync::Arc;

use openssl::pkey::PKey;
use openssl::pkey::Private;
use openssl::x509::X509Ref;
use openssl::x509::X509;

use self::cache::CachedCertificate;
use self::cache::MemoizedCertificate;
use crate::is_configuration::IsConfiguration;

pub mod cache;
pub mod pem;
pub mod rustls_config;
pub mod tls_connector;

pub trait CertificateConfig: IsConfiguration {
    type Error: std::error::Error;
    fn intermediates(&self) -> Result<Arc<Vec<X509>>, Self::Error>;
    fn certificate(&self) -> Result<Arc<Certificate>, Self::Error>;

    fn memoize(self) -> MemoizedCertificate<Self>
    where
        Self: Sized,
    {
        MemoizedCertificate::new(self)
    }

    fn cache(self) -> Result<CachedCertificate<MemoizedCertificate<Self>>, Self::Error>
    where
        Self: Sized,
    {
        CachedCertificate::new(self.memoize())
    }
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

impl<T: CertificateConfig> CertificateConfig for Arc<T> {
    type Error = T::Error;

    fn intermediates(&self) -> Result<Arc<Vec<X509>>, Self::Error> {
        self.as_ref().intermediates()
    }

    fn certificate(&self) -> Result<Arc<Certificate>, Self::Error> {
        self.as_ref().certificate()
    }
}
