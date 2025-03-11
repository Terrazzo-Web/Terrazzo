use std::convert::Infallible;
use std::sync::Arc;

use openssl::x509::X509;

use super::CertificateConfig;
use super::X509CertificateInfo;
use crate::security_configuration::common::get_or_init;

pub struct MemoizedCertificate<C> {
    base: C,
    intermediates: std::sync::Mutex<Option<Arc<Vec<X509>>>>,
    certificate: std::sync::Mutex<Option<Arc<X509CertificateInfo>>>,
}

impl<C> MemoizedCertificate<C> {
    pub fn new(base: C) -> Self {
        Self {
            base,
            intermediates: Default::default(),
            certificate: Default::default(),
        }
    }
}

impl<C: CertificateConfig> CertificateConfig for MemoizedCertificate<C> {
    type Error = C::Error;

    fn intermediates(&self) -> Result<Arc<Vec<X509>>, Self::Error> {
        get_or_init(&self.intermediates, || self.base.intermediates())
    }

    fn certificate(&self) -> Result<Arc<X509CertificateInfo>, Self::Error> {
        get_or_init(&self.certificate, || self.base.certificate())
    }
}

#[derive(Clone)]
pub struct CachedCertificate {
    intermediates: Arc<Vec<X509>>,
    certificate: Arc<X509CertificateInfo>,
}

impl CachedCertificate {
    pub fn new<C: CertificateConfig>(base: C) -> Result<Self, C::Error> {
        Ok(Self {
            intermediates: base.intermediates()?,
            certificate: base.certificate()?,
        })
    }
}

impl CertificateConfig for CachedCertificate {
    type Error = Infallible;

    fn intermediates(&self) -> Result<Arc<Vec<X509>>, Self::Error> {
        Ok(self.intermediates.clone())
    }

    fn certificate(&self) -> Result<Arc<X509CertificateInfo>, Self::Error> {
        Ok(self.certificate.clone())
    }
}

mod debug {
    use super::CachedCertificate;

    impl std::fmt::Debug for CachedCertificate {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("CachedCertificate")
                .field("intermediates", &self.intermediates)
                .field("certificate", &self.certificate)
                .finish()
        }
    }
}
