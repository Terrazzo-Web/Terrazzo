use std::sync::Arc;

use openssl::x509::X509;

use super::CertificateConfig;
use super::X509CertificateInfo;
use crate::dynamic_config::DynamicConfig;

/// A [CertificateConfig] that is dynamic.
pub struct DynamicCertificate<C>(Arc<DynamicConfig<C>>);

impl<C> From<Arc<DynamicConfig<C>>> for DynamicCertificate<C> {
    fn from(value: Arc<DynamicConfig<C>>) -> Self {
        Self(value)
    }
}

impl<C: CertificateConfig> CertificateConfig for DynamicCertificate<C> {
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
