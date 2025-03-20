use std::convert::Infallible;
use std::sync::Arc;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use openssl::x509::store::X509Store;
use openssl::x509::store::X509StoreBuilder;

use super::CertificateConfig;
use super::cache::CachedCertificate;
use super::pem::PemCertificate;
use super::pem::PemCertificateError;
use crate::security_configuration::trusted_store::TrustedStoreConfig;

/// [PemCertificate] is both
///   - primarily a certificate+intermediates, that is [CertificateConfig],
///   - can also be used as a root CA [TrustedStoreConfig].
///
/// In the latter case, the list of intermediates is empty,
/// the [PemCertificate] represents a single self-signed CA.
impl TrustedStoreConfig for PemCertificate {
    type Error = AsTrustedStoreError<PemCertificateError>;
    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        as_trusted_store(self)
    }
}

impl TrustedStoreConfig for CachedCertificate {
    type Error = AsTrustedStoreError<Infallible>;

    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        as_trusted_store(self)
    }
}

pub fn as_trusted_store<C: CertificateConfig>(
    certificate: &C,
) -> Result<Arc<X509Store>, AsTrustedStoreError<C::Error>> {
    let mut trusted_roots =
        X509StoreBuilder::new().map_err(AsTrustedStoreError::X509StoreBuilder)?;
    trusted_roots
        .add_cert(
            certificate
                .certificate()
                .map_err(AsTrustedStoreError::Certificate)?
                .certificate
                .clone(),
        )
        .map_err(AsTrustedStoreError::AddCert)?;
    Ok(Arc::new(trusted_roots.build()))
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum AsTrustedStoreError<E: std::error::Error> {
    #[error("[{n}] Failed to create X509StoreBuilder: {0}", n = self.name())]
    X509StoreBuilder(ErrorStack),

    /// The [CertificateConfig] failed to generate an [openssl::x509::X509].
    #[error("[{n}] {0}", n = self.name())]
    Certificate(E),

    #[error("[{n}] Failed to add a X509 certificate to the store: {0}", n = self.name())]
    AddCert(ErrorStack),
}
