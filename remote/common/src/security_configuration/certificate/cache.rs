use std::marker::PhantomData;
use std::sync::Arc;

use openssl::x509::X509;

use super::Certificate;
use super::CertificateConfig;
use crate::security_configuration::common::get_or_init;

pub struct MemoizedCertificate<C> {
    base: C,
    intermediates: std::sync::Mutex<Option<Arc<Vec<X509>>>>,
    certificate: std::sync::Mutex<Option<Arc<Certificate>>>,
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

    fn certificate(&self) -> Result<Arc<Certificate>, Self::Error> {
        get_or_init(&self.certificate, || self.base.certificate())
    }
}

pub struct CachedCertificate<C> {
    _base: PhantomData<C>,
    intermediates: Arc<Vec<X509>>,
    certificate: Arc<Certificate>,
}

impl<C: CertificateConfig> CachedCertificate<C> {
    pub fn new(base: C) -> Result<Self, C::Error> {
        Ok(Self {
            _base: PhantomData,
            intermediates: base.intermediates()?,
            certificate: base.certificate()?,
        })
    }
}

impl<C: CertificateConfig> CertificateConfig for CachedCertificate<C> {
    type Error = C::Error;

    fn intermediates(&self) -> Result<Arc<Vec<X509>>, C::Error> {
        Ok(self.intermediates.clone())
    }

    fn certificate(&self) -> Result<Arc<Certificate>, C::Error> {
        Ok(self.certificate.clone())
    }
}
