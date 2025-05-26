use std::convert::Infallible;
use std::sync::Arc;

use openssl::x509::store::X509Store;

use super::TrustedStoreConfig;
use crate::x509::native_roots::native_roots;

/// The [TrustedStoreConfig] for certificates trusted by the OS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeTrustedStoreConfig;

impl TrustedStoreConfig for NativeTrustedStoreConfig {
    type Error = Infallible;
    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        Ok(native_roots().clone())
    }
}
