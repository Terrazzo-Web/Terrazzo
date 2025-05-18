use std::sync::Arc;

use openssl::x509::X509;
use openssl::x509::store::X509Store;

use super::TrustedStoreConfig;
use super::certificate::CertificateConfig;
use crate::certificate_info::X509CertificateInfo;

#[derive(Clone, Debug)]
pub enum EitherConfig<L, R> {
    Left(L),
    Right(R),
}

impl<L: TrustedStoreConfig, R: TrustedStoreConfig> TrustedStoreConfig for EitherConfig<L, R> {
    type Error = EitherConfig<L::Error, R::Error>;

    fn root_certificates(&self) -> Result<Arc<X509Store>, Self::Error> {
        match self {
            Self::Left(store) => store.root_certificates().map_err(EitherConfig::Left),
            Self::Right(store) => store.root_certificates().map_err(EitherConfig::Right),
        }
    }
}

impl<L: CertificateConfig, R: CertificateConfig> CertificateConfig for EitherConfig<L, R> {
    type Error = EitherConfig<L::Error, R::Error>;

    fn intermediates(&self) -> Result<Arc<Vec<X509>>, Self::Error> {
        match self {
            Self::Left(store) => store.intermediates().map_err(EitherConfig::Left),
            Self::Right(store) => store.intermediates().map_err(EitherConfig::Right),
        }
    }

    fn certificate(&self) -> Result<Arc<X509CertificateInfo>, Self::Error> {
        match self {
            Self::Left(store) => store.certificate().map_err(EitherConfig::Left),
            Self::Right(store) => store.certificate().map_err(EitherConfig::Right),
        }
    }
}

impl<L: std::error::Error, R: std::error::Error> std::error::Error for EitherConfig<L, R> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Left(s) => s.source(),
            Self::Right(s) => s.source(),
        }
    }
}

impl<L: std::fmt::Display, R: std::fmt::Display> std::fmt::Display for EitherConfig<L, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Left(s) => s.fmt(f),
            Self::Right(s) => s.fmt(f),
        }
    }
}
