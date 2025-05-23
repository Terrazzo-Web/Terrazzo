use std::sync::Arc;

use openssl::x509::X509;
use openssl::x509::store::X509Store;

use super::CertificateConfig;
use super::X509CertificateInfo;
use crate::dynamic_config::DynamicConfig;
use crate::dynamic_config::mode::Mode;
use crate::security_configuration::trusted_store::TrustedStoreConfig;

/// A [CertificateConfig] that is dynamic.
pub struct DynamicCertificate<C, M: Mode>(Arc<DynamicConfig<C, M>>);

impl<C, M: Mode> From<Arc<DynamicConfig<C, M>>> for DynamicCertificate<C, M> {
    fn from(value: Arc<DynamicConfig<C, M>>) -> Self {
        Self(value)
    }
}

impl<C: CertificateConfig, M: Mode> CertificateConfig for DynamicCertificate<C, M> {
    type Error = C::Error;

    fn intermediates(&self) -> Result<Arc<Vec<X509>>, Self::Error> {
        self.0.with(|config| config.intermediates())
    }

    fn certificate(&self) -> Result<Arc<X509CertificateInfo>, Self::Error> {
        self.0.with(|config| config.certificate())
    }

    fn is_dynamic(&self) -> bool {
        true
    }
}

impl<C: TrustedStoreConfig, M: Mode> TrustedStoreConfig for DynamicCertificate<C, M> {
    type Error = C::Error;

    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        self.0.with(|config| config.root_certificates())
    }
}

impl<C: Clone + std::fmt::Debug, M: Mode> std::fmt::Debug for DynamicCertificate<C, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DynamicCertificate")
            .field(&self.0.get())
            .finish()
    }
}
