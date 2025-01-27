use std::sync::Arc;

use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::error::ErrorStack;
use openssl::x509::store::X509Store;
use openssl::x509::store::X509StoreBuilder;

use super::common::parse_pem_certificates;

pub trait TrustedStoreConfig {
    type Error: std::error::Error;
    fn root_certificates(&self) -> Result<X509Store, Self::Error>;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PemTrustedStore {
    pub root_certificates_pem: String,
}

impl TrustedStoreConfig for PemTrustedStore {
    type Error = PemTrustedStoreError;

    fn root_certificates(&self) -> Result<X509Store, Self::Error> {
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
        Ok(trusted_roots.build())
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

impl<T: TrustedStoreConfig> TrustedStoreConfig for Arc<T> {
    type Error = T::Error;

    fn root_certificates(&self) -> Result<X509Store, Self::Error> {
        self.as_ref().root_certificates()
    }
}
