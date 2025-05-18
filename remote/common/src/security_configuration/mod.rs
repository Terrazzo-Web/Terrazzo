use std::sync::Arc;

use openssl::x509::X509;
use openssl::x509::store::X509Store;

use self::certificate::CertificateConfig;
use self::trusted_store::TrustedStoreConfig;
use crate::certificate_info::X509CertificateInfo;
use crate::is_global::IsGlobal;

pub mod certificate;
mod common;
pub mod custom_server_certificate_verifier;
pub mod either;
pub mod trusted_store;

/// A security config has both a trusted store and a client certificate.
///
/// - This is used to configure the client certificate issuer: we need both
///     1. When issuing client certificates: the certificate (+intermediates)
///        to sign the CMS extension.
///     2. When validating client certificates: the trusted roots to validate
///        the issuing cert in the signed CMS extension
/// - This is also used to configure the Root CA
///     - The Root CA doesn't need to be secure. It's just there to sanity
///       check client certs are owned by us before the custom validation logic
///       kicks in.
///     - The Root CA is the issuer of client certificates. Technically we need
///       an issuer, but the real validation is the CMS extension.
#[derive(Clone, Debug)]
pub struct SecurityConfig<T, C> {
    pub trusted_store: T,
    pub certificate: C,
}

impl<T: TrustedStoreConfig, C: IsGlobal> TrustedStoreConfig for SecurityConfig<T, C> {
    type Error = T::Error;

    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        self.trusted_store.root_certificates()
    }
}

impl<T: IsGlobal, C: CertificateConfig> CertificateConfig for SecurityConfig<T, C> {
    type Error = C::Error;

    fn intermediates(&self) -> Result<Arc<Vec<X509>>, Self::Error> {
        self.certificate.intermediates()
    }

    fn certificate(&self) -> Result<Arc<X509CertificateInfo>, Self::Error> {
        self.certificate.certificate()
    }
}

pub trait HasSecurityConfig: TrustedStoreConfig + CertificateConfig {}
impl<T: TrustedStoreConfig + CertificateConfig> HasSecurityConfig for T {}
