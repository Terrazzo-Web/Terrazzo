use std::sync::Arc;

use openssl::x509::store::X509Store;

use crate::is_global::IsGlobal;
use crate::is_global::IsGlobalError;

pub mod cache;
pub mod empty;
pub mod load;
pub mod native;
pub mod pem;
pub mod root_cert_store;
pub mod tls_client;

/// Trait for configuration that holds a [X509Store].
pub trait TrustedStoreConfig: IsGlobal {
    type Error: IsGlobalError;
    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error>;
}

impl<T: TrustedStoreConfig> TrustedStoreConfig for Arc<T> {
    type Error = T::Error;

    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        self.as_ref().root_certificates()
    }
}
