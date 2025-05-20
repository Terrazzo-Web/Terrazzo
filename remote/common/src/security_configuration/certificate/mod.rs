use std::sync::Arc;

use openssl::x509::X509;
use openssl::x509::X509Ref;

use self::cache::CachedCertificate;
use self::cache::MemoizedCertificate;
use crate::certificate_info::X509CertificateInfo;
use crate::is_global::IsGlobal;

pub mod as_trusted_store;
pub mod cache;
pub mod dynamic;
pub mod pem;
pub mod tls_server;

/// Trait for X509 certificate along with the intermediates.
pub trait CertificateConfig: IsGlobal {
    type Error: std::error::Error;

    /// Computes the list of intermediate certificates.
    fn intermediates(&self) -> Result<Arc<Vec<X509>>, Self::Error>;

    /// Computes the X509 leaf certificate
    fn certificate(&self) -> Result<Arc<X509CertificateInfo>, Self::Error>;

    /// Whether the certificate can change over time, ie Let's Encrypt certificates.
    fn is_dynamic(&self) -> bool {
        false
    }

    /// Returns a memoized [CertificateConfig].
    fn memoize(self) -> MemoizedCertificate<Self>
    where
        Self: Sized,
    {
        MemoizedCertificate::new(self)
    }

    /// Returns a cached [CertificateConfig].
    fn cache(self) -> Result<CachedCertificate, Self::Error>
    where
        Self: Sized,
    {
        CachedCertificate::new(self.memoize())
    }
}

impl X509CertificateInfo {
    /// Prints a textual representation of a certificate.
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

    fn certificate(&self) -> Result<Arc<X509CertificateInfo>, Self::Error> {
        self.as_ref().certificate()
    }

    fn is_dynamic(&self) -> bool {
        self.as_ref().is_dynamic()
    }
}
