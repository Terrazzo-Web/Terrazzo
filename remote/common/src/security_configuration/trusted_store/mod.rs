use std::sync::Arc;

use openssl::x509::store::X509Store;

use crate::is_configuration::IsConfiguration;

pub mod cache;
pub mod empty;
pub mod native;
pub mod pem;
pub mod root_cert_store;

pub trait TrustedStoreConfig: IsConfiguration {
    type Error: std::error::Error + 'static;
    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error>;
}

impl<T: TrustedStoreConfig> TrustedStoreConfig for Arc<T> {
    type Error = T::Error;

    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        self.as_ref().root_certificates()
    }
}
