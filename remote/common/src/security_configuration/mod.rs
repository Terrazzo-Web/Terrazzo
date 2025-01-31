use std::sync::Arc;

use openssl::x509::store::X509Store;
use openssl::x509::X509;

use self::certificate::Certificate;
use self::certificate::CertificateConfig;
use self::trusted_store::TrustedStoreConfig;

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

    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        self.trusted_store.root_certificates()
    }
}

impl<T, C: CertificateConfig> CertificateConfig for SecurityConfig<T, C> {
    type Error = C::Error;

    fn intermediates(&self) -> Result<Arc<Vec<X509>>, Self::Error> {
        self.certificate.intermediates()
    }

    fn certificate(&self) -> Result<Arc<Certificate>, Self::Error> {
        self.certificate.certificate()
    }
}

pub trait HasSecurityConfig: TrustedStoreConfig + CertificateConfig {}
impl<T: TrustedStoreConfig, C: CertificateConfig> HasSecurityConfig for SecurityConfig<T, C> {}

impl<T: HasSecurityConfig> HasSecurityConfig for Arc<T> {}
