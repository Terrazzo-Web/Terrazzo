use std::sync::Arc;

use openssl::x509::store::X509Store;

pub mod cache;
pub mod empty;
pub mod native;
pub mod pem;

pub trait TrustedStoreConfig {
    type Error: std::error::Error + 'static;
    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error>;
}

impl<T: TrustedStoreConfig> TrustedStoreConfig for Arc<T> {
    type Error = T::Error;

    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        self.as_ref().root_certificates()
    }
}
