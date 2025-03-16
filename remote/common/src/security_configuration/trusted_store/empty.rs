use std::convert::Infallible;
use std::sync::Arc;
use std::sync::OnceLock;

use openssl::x509::store::X509Store;
use openssl::x509::store::X509StoreBuilder;

use super::TrustedStoreConfig;

/// A [TrustedStoreConfig] that doesn't contain any trusted certificates.
pub struct EmptyTrustedStoreConfig;

impl TrustedStoreConfig for EmptyTrustedStoreConfig {
    type Error = Infallible;

    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        static EMPTY: OnceLock<Arc<X509Store>> = OnceLock::new();
        Ok(EMPTY
            .get_or_init(|| {
                X509StoreBuilder::new()
                    .expect("X509StoreBuilder::new()")
                    .build()
                    .into()
            })
            .clone())
    }
}
