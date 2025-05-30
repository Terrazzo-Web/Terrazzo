use std::sync::Arc;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use openssl::x509::store::X509Store;
use openssl::x509::store::X509StoreBuilder;

use super::TrustedStoreConfig;
use crate::security_configuration::common::parse_pem_certificates;

/// A [TrustedStoreConfig] based on PEM files stored on disk.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PemTrustedStore {
    pub root_certificates_pem: String,
}

impl TrustedStoreConfig for PemTrustedStore {
    type Error = PemTrustedStoreError;

    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        let mut trusted_roots =
            X509StoreBuilder::new().map_err(PemTrustedStoreError::X509StoreBuilder)?;
        let root_certificates = parse_pem_certificates(&self.root_certificates_pem);
        for root_certificate in root_certificates {
            match root_certificate {
                Ok(root_ca) => trusted_roots
                    .add_cert(root_ca)
                    .map_err(PemTrustedStoreError::AddCert)?,
                Err(error) => tracing::trace!("Failed to parse Root CA: {error}"),
            }
        }
        Ok(Arc::new(trusted_roots.build()))
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum PemTrustedStoreError {
    #[error("[{n}] Failed to create X509StoreBuilder: {0}", n = self.name())]
    X509StoreBuilder(ErrorStack),

    #[error("[{n}] Failed to add a X509 certificate to the store: {0}", n = self.name())]
    AddCert(ErrorStack),
}
