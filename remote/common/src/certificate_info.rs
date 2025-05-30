//! Utils for certificate + private key.

use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::pkey::PKey;
use openssl::pkey::PKeyRef;
use openssl::pkey::Private;
use openssl::x509::X509;
use openssl::x509::X509Ref;

/// Represents a certificate + private key.
///
/// The type is generic so it can be reused in the following various scenarios:
/// - A pair of files reprenting the certificate + private key
/// - A certificate and private key represented as PEM strings
/// - A [X509] + [PKey] openssl objects
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CertificateInfo<X, Y = X> {
    pub certificate: X,
    pub private_key: Y,
}

impl<X> CertificateInfo<X> {
    /// Applies a transformation to the certificate and private key.
    pub fn map<F: Fn(X) -> Y, Y>(self, f: F) -> CertificateInfo<Y> {
        CertificateInfo {
            certificate: f(self.certificate),
            private_key: f(self.private_key),
        }
    }

    /// Applies a fallible transformation to the certificate and private key.
    pub fn try_map<F: Fn(X) -> Result<Y, E>, Y, E: std::error::Error>(
        self,
        f: F,
    ) -> Result<CertificateInfo<Y>, CertificateError<E>> {
        Ok(CertificateInfo {
            certificate: f(self.certificate).map_err(CertificateError::Certificate)?,
            private_key: f(self.private_key).map_err(CertificateError::PrivateKey)?,
        })
    }
}

impl<X, Y> CertificateInfo<X, Y> {
    /// Zips two [CertificateInfo] into a single struct containing tuples.
    pub fn zip<XX, YY>(self, other: CertificateInfo<XX, YY>) -> CertificateInfo<(X, XX), (Y, YY)> {
        CertificateInfo {
            certificate: (self.certificate, other.certificate),
            private_key: (self.private_key, other.private_key),
        }
    }
}

impl<X, Y> CertificateInfo<X, Y> {
    /// Converts from `&CertificateInfo<T>` to `CertificateInfo<&T>`.
    ///
    /// Similar to [Option::as_ref]
    pub fn as_ref<XX, YY>(&self) -> CertificateInfo<&XX, &YY>
    where
        X: AsRef<XX>,
        Y: AsRef<YY>,
        XX: ?Sized,
        YY: ?Sized,
    {
        CertificateInfo {
            certificate: self.certificate.as_ref(),
            private_key: self.private_key.as_ref(),
        }
    }
}

/// Error type for [CertificateInfo::try_map].
#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum CertificateError<E: std::error::Error> {
    #[error("[{n}] {0}", n = self.name())]
    Certificate(E),

    #[error("[{n}] {0}", n = self.name())]
    PrivateKey(E),
}

/// Alias for [CertificateInfo] containing an OpenSSL certificate.
pub type X509CertificateInfo = CertificateInfo<X509, PKey<Private>>;

/// Alias for [CertificateInfo] containing an OpenSSL certificate ref.
pub type X509CertificateInfoRef<'t> = CertificateInfo<&'t X509Ref, &'t PKeyRef<Private>>;
