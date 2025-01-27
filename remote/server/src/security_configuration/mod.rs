use std::sync::Arc;

use certificate::CertificateConfig;
use openssl::x509::store::X509Store;
use trusted_store::TrustedStoreConfig;

use self::certificate::Certificate;

pub mod certificate;
mod common;
pub mod trusted_store;

#[derive(Debug)]
pub struct SecurityConfig<T, C> {
    pub trusted_store: T,
    pub certificate: C,
}

impl<T: TrustedStoreConfig, C> TrustedStoreConfig for SecurityConfig<T, C> {
    type Error = T::Error;

    fn root_certificates(&self) -> Result<X509Store, Self::Error> {
        self.trusted_store.root_certificates()
    }
}

impl<T, C: CertificateConfig> CertificateConfig for SecurityConfig<T, C> {
    type Error = C::Error;

    fn intermediates(&self) -> Result<Vec<openssl::x509::X509>, Self::Error> {
        self.certificate.intermediates()
    }

    fn certificate(&self) -> Result<Certificate, Self::Error> {
        self.certificate.certificate()
    }
}

pub trait HasSecurityConfig: TrustedStoreConfig + CertificateConfig {}
impl<T: TrustedStoreConfig, C: CertificateConfig> HasSecurityConfig for SecurityConfig<T, C> {}

impl<T: HasSecurityConfig> HasSecurityConfig for Arc<T> {}
