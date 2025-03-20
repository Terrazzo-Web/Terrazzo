use nameth::NamedEnumValues as _;
use nameth::nameth;

use super::cache::CachedTrustedStoreConfig;
use super::native::NativeTrustedStoreConfig;
use super::pem::PemTrustedStore;
use super::pem::PemTrustedStoreError;
use crate::unwrap_infallible::UnwrapInfallible as _;

pub enum LoadTrustedStore {
    Native,
    PEM(String),
}

impl LoadTrustedStore {
    pub fn load(&self) -> Result<CachedTrustedStoreConfig, LoadTrustedStoreError> {
        match self {
            LoadTrustedStore::Native => {
                Ok(CachedTrustedStoreConfig::new(NativeTrustedStoreConfig).unwrap_infallible())
            }
            LoadTrustedStore::PEM(root_certificates_pem) => {
                Ok(CachedTrustedStoreConfig::new(PemTrustedStore {
                    root_certificates_pem: root_certificates_pem.to_owned(),
                })?)
            }
        }
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum LoadTrustedStoreError {
    #[error("[{n}] {0}", n = self.name())]
    LoadPem(#[from] PemTrustedStoreError),
}
