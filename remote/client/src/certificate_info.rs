use nameth::nameth;
use nameth::NamedEnumValues as _;

#[derive(Clone, Copy, Debug, Default)]
pub struct CertificateInfo<T> {
    pub certificate: T,
    pub private_key: T,
}

impl<T> CertificateInfo<T> {
    pub fn map<F: Fn(T) -> U, U>(self, f: F) -> CertificateInfo<U> {
        CertificateInfo {
            certificate: f(self.certificate),
            private_key: f(self.private_key),
        }
    }

    pub fn try_map<F: Fn(T) -> Result<U, E>, U, E: std::error::Error>(
        self,
        f: F,
    ) -> Result<CertificateInfo<U>, CertificateError<E>> {
        Ok(CertificateInfo {
            certificate: f(self.certificate).map_err(CertificateError::Certificate)?,
            private_key: f(self.private_key).map_err(CertificateError::PrivateKey)?,
        })
    }

    pub fn zip<U>(self, other: CertificateInfo<U>) -> CertificateInfo<(T, U)> {
        CertificateInfo {
            certificate: (self.certificate, other.certificate),
            private_key: (self.private_key, other.private_key),
        }
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum CertificateError<E: std::error::Error> {
    #[error("[{n}] {0}", n = self.name())]
    Certificate(E),

    #[error("[{n}] {0}", n = self.name())]
    PrivateKey(E),
}
