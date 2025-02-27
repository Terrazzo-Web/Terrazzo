use std::convert::Infallible;
use std::sync::Arc;

use openssl::x509::store::X509Store;

use super::TrustedStoreConfig;
use crate::security_configuration::common::get_or_init;

pub struct MemoizedTrustedStoreConfig<C> {
    base: C,
    root_certificates: std::sync::Mutex<Option<Arc<X509Store>>>,
}

impl<C> MemoizedTrustedStoreConfig<C> {
    pub fn new(base: C) -> Self {
        Self {
            base,
            root_certificates: Default::default(),
        }
    }
}

impl<C: TrustedStoreConfig> TrustedStoreConfig for MemoizedTrustedStoreConfig<C> {
    type Error = C::Error;
    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        get_or_init(&self.root_certificates, || self.base.root_certificates())
    }
}

pub struct CachedTrustedStoreConfig {
    root_certificates: Arc<X509Store>,
}

impl CachedTrustedStoreConfig {
    pub fn new<C: TrustedStoreConfig>(base: C) -> Result<Self, C::Error> {
        Ok(Self {
            root_certificates: base.root_certificates()?,
        })
    }
}

impl TrustedStoreConfig for CachedTrustedStoreConfig {
    type Error = Infallible;
    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        Ok(self.root_certificates.clone())
    }
}

impl From<X509Store> for CachedTrustedStoreConfig {
    fn from(root_certificates: X509Store) -> Self {
        CachedTrustedStoreConfig {
            root_certificates: Arc::from(root_certificates),
        }
    }
}
