use std::sync::Arc;

use openssl::x509::X509;
use openssl::x509::store::X509Store;

use self::certificate::CertificateConfig;
use self::trusted_store::TrustedStoreConfig;
use crate::certificate_info::X509CertificateInfo;
use crate::is_global::IsGlobal;

pub mod certificate;
mod common;
pub mod custom_server_certificate_verifier;
pub mod trusted_store;

#[derive(Clone, Debug)]
pub struct SecurityConfig<T, C> {
    pub trusted_store: T,
    pub certificate: C,
}

impl<T: TrustedStoreConfig, C: IsGlobal> TrustedStoreConfig for SecurityConfig<T, C> {
    type Error = T::Error;

    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        self.trusted_store.root_certificates()
    }
}

impl<T: IsGlobal, C: CertificateConfig> CertificateConfig for SecurityConfig<T, C> {
    type Error = C::Error;

    fn intermediates(&self) -> Result<Arc<Vec<X509>>, Self::Error> {
        self.certificate.intermediates()
    }

    fn certificate(&self) -> Result<Arc<X509CertificateInfo>, Self::Error> {
        self.certificate.certificate()
    }
}

pub trait HasSecurityConfig: TrustedStoreConfig + CertificateConfig {}
impl<T: TrustedStoreConfig + CertificateConfig> HasSecurityConfig for T {}
