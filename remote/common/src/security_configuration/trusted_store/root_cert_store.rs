use std::future::Future;

use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::error::ErrorStack;
use rustls::pki_types::CertificateDer;
use rustls::RootCertStore;

use super::TrustedStoreConfig;

pub trait ToRootCertStore: TrustedStoreConfig {
    fn to_root_cert_store(
        &self,
    ) -> impl Future<Output = Result<RootCertStore, ToRustlsRootCertStoreError<Self::Error>>> {
        to_root_cert_store_impl(self)
    }
}

impl<T: TrustedStoreConfig> ToRootCertStore for T {}

pub async fn to_root_cert_store_impl<T: TrustedStoreConfig + ?Sized>(
    trusted_store: &T,
) -> Result<RootCertStore, ToRustlsRootCertStoreError<T::Error>> {
    let mut roots = RootCertStore::empty();
    let trusted_roots = trusted_store
        .root_certificates()
        .map_err(ToRustlsRootCertStoreError::Certificate)?;
    for trusted_root in trusted_roots.all_certificates() {
        let trusted_root_der = trusted_root
            .to_der()
            .map_err(ToRustlsRootCertStoreError::X509ToDer)?;
        roots
            .add(CertificateDer::from_slice(&trusted_root_der))
            .map_err(ToRustlsRootCertStoreError::AddCertificate)?;
    }
    Ok(roots)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ToRustlsRootCertStoreError<E: std::error::Error> {
    #[error("[{n}] {0}", n = self.name())]
    Certificate(E),

    #[error("[{n}] {0}", n = self.name())]
    X509ToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    AddCertificate(rustls::Error),
}
