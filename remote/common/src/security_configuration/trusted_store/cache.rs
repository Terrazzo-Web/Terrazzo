use std::marker::PhantomData;
use std::sync::Arc;

use openssl::x509::store::X509Store;

use super::empty::EmptyTrustedStoreConfig;
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

pub struct CachedTrustedStoreConfig<C> {
    _base: PhantomData<C>,
    root_certificates: Arc<X509Store>,
}

impl<C: TrustedStoreConfig> CachedTrustedStoreConfig<C> {
    pub fn new(base: C) -> Result<Self, C::Error> {
        Ok(Self {
            _base: PhantomData,
            root_certificates: base.root_certificates()?,
        })
    }
}

impl<C: TrustedStoreConfig> TrustedStoreConfig for CachedTrustedStoreConfig<C> {
    type Error = C::Error;
    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        Ok(self.root_certificates.clone())
    }
}

impl From<X509Store> for CachedTrustedStoreConfig<EmptyTrustedStoreConfig> {
    fn from(root_certificates: X509Store) -> Self {
        CachedTrustedStoreConfig {
            _base: PhantomData,
            root_certificates: Arc::from(root_certificates),
        }
    }
}
