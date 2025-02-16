use nameth::nameth;
use nameth::NamedEnumValues as _;

#[derive(Clone, Copy, Debug, Default)]
pub struct CertificateInfo<X, Y = X> {
    pub certificate: X,
    pub private_key: Y,
}

impl<X> CertificateInfo<X> {
    pub fn map<F: Fn(X) -> Y, Y>(self, f: F) -> CertificateInfo<Y> {
        CertificateInfo {
            certificate: f(self.certificate),
            private_key: f(self.private_key),
        }
    }

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
    pub fn zip<XX, YY>(self, other: CertificateInfo<XX, YY>) -> CertificateInfo<(X, XX), (Y, YY)> {
        CertificateInfo {
            certificate: (self.certificate, other.certificate),
            private_key: (self.private_key, other.private_key),
        }
    }
}

impl<X, Y> CertificateInfo<X, Y> {
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

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum CertificateError<E: std::error::Error> {
    #[error("[{n}] {0}", n = self.name())]
    Certificate(E),

    #[error("[{n}] {0}", n = self.name())]
    PrivateKey(E),
}
