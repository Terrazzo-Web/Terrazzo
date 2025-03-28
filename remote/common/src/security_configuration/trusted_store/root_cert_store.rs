use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use rustls::RootCertStore;
use rustls::pki_types::CertificateDer;

use super::TrustedStoreConfig;

/// Create a RusTLS [RootCertStore] from a [TrustedStoreConfig].
///
/// This is used by [super::tls_client::ToTlsClient].
pub trait ToRootCertStore: TrustedStoreConfig {
    fn to_root_cert_store(&self) -> Result<RootCertStore, ToRootCertStoreError<Self::Error>> {
        to_root_cert_store_impl(self)
    }
}

impl<T: TrustedStoreConfig> ToRootCertStore for T {}

pub fn to_root_cert_store_impl<T: TrustedStoreConfig + ?Sized>(
    trusted_store: &T,
) -> Result<RootCertStore, ToRootCertStoreError<T::Error>> {
    let mut roots = RootCertStore::empty();
    let trusted_roots = trusted_store
        .root_certificates()
        .map_err(ToRootCertStoreError::RootCertificates)?;
    for trusted_root in trusted_roots.all_certificates() {
        let trusted_root_der = trusted_root
            .to_der()
            .map_err(ToRootCertStoreError::X509ToDer)?;
        roots
            .add(CertificateDer::from_slice(&trusted_root_der))
            .map_err(ToRootCertStoreError::AddCertificate)?;
    }
    Ok(roots)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ToRootCertStoreError<E: std::error::Error> {
    #[error("[{n}] {0}", n = self.name())]
    RootCertificates(E),

    #[error("[{n}] {0}", n = self.name())]
    X509ToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    AddCertificate(rustls::Error),
}
