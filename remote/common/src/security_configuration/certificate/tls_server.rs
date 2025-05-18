use std::sync::Arc;
use std::sync::Mutex;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use openssl::x509::X509;
use rustls::ServerConfig;
use tracing::debug;
use tracing::info;
use tracing::info_span;

use super::CertificateConfig;
use crate::certificate_info::X509CertificateInfo;
use crate::security_configuration::certificate::display_x509_certificate;

/// Create a RusTLS [ServerConfig] from a [CertificateConfig].
pub trait ToTlsServer<E: std::error::Error> {
    fn to_tls_server(&self) -> Result<Arc<ServerConfig>, ToTlsServerError<E>>;

    fn dynamic(self) -> DynamicTlsServer<Self>
    where
        Self: CertificateConfig + Sized,
    {
        DynamicTlsServer {
            certificate_config: self,
            cache: Mutex::default(),
        }
    }
}

impl<T: CertificateConfig> ToTlsServer<T::Error> for T {
    fn to_tls_server(&self) -> Result<Arc<ServerConfig>, ToTlsServerError<T::Error>> {
        let certificate = self.certificate().map_err(ToTlsServerError::Certificate)?;
        let intermediates = self
            .intermediates()
            .map_err(ToTlsServerError::Intermediates)?;
        to_tls_server_impl::<Self>(certificate, intermediates)
    }
}

fn to_tls_server_impl<T: CertificateConfig + ?Sized>(
    certificate: Arc<X509CertificateInfo>,
    intermediates: Arc<Vec<X509>>,
) -> Result<Arc<ServerConfig>, ToTlsServerError<T::Error>> {
    let _span = info_span!("Setup TLS server certificate").entered();
    let mut certificate_chain = vec![];
    {
        info!(
            "Server certificate: {:?} issued by {:?}",
            certificate.certificate.subject_name(),
            certificate.certificate.issuer_name()
        );
        debug!("Server certificate details:  {}", certificate.display());
        let certificate = certificate.certificate.to_der();
        let certificate = certificate.map_err(ToTlsServerError::CertificateToDer)?;
        certificate_chain.push(certificate.into());
    }
    for intermediate in intermediates.iter() {
        info!(
            "Intermediate certificate: {:?} issued by {:?}",
            intermediate.subject_name(),
            intermediate.issuer_name()
        );
        debug!(
            "Intermediate certificate details:  {}",
            display_x509_certificate(intermediate)
        );
        let intermediate = intermediate.to_der();
        let intermediate = intermediate.map_err(ToTlsServerError::IntermediateToDer)?;
        certificate_chain.push(intermediate.into());
    }

    let private_key = certificate
        .private_key
        .private_key_to_der()
        .map_err(ToTlsServerError::PrivateKeyToDer)?
        .try_into()
        .map_err(ToTlsServerError::ToPrivateKey)?;

    let mut server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(cert_resolver)
        .with_single_cert(certificate_chain, private_key)
        .map_err(ToTlsServerError::ServerConfig)?;
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(Arc::new(server_config))
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ToTlsServerError<E: std::error::Error> {
    #[error("[{n}] {0}", n = self.name())]
    Certificate(E),

    #[error("[{n}] {0}", n = self.name())]
    CertificateToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    Intermediates(E),

    #[error("[{n}] {0}", n = self.name())]
    IntermediateToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    PrivateKeyToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    ToPrivateKey(&'static str),

    #[error("[{n}] {0}", n = self.name())]
    ServerConfig(rustls::Error),
}

pub struct DynamicTlsServer<T: CertificateConfig> {
    certificate_config: T,
    cache: std::sync::Mutex<Option<DynamicTlsServerState>>,
}

struct DynamicTlsServerState {
    certificate: Arc<X509CertificateInfo>,
    intermediates: Arc<Vec<X509>>,
    server_config: Arc<ServerConfig>,
}

impl<T: CertificateConfig> ToTlsServer<T::Error> for DynamicTlsServer<T> {
    fn to_tls_server(&self) -> Result<Arc<ServerConfig>, ToTlsServerError<T::Error>> {
        let certificate = self
            .certificate_config
            .certificate()
            .map_err(ToTlsServerError::Certificate)?;
        let intermediates = self
            .certificate_config
            .intermediates()
            .map_err(ToTlsServerError::Intermediates)?;

        let mut lock = self.cache.lock().unwrap();
        if let Some(cache) = &*lock {
            if Arc::ptr_eq(&cache.certificate, &certificate)
                && Arc::ptr_eq(&cache.intermediates, &intermediates)
            {
                return Ok(cache.server_config.clone());
            }
        }

        to_tls_server_impl::<T>(certificate.clone(), intermediates.clone()).inspect(
            |server_config| {
                *lock = Some(DynamicTlsServerState {
                    certificate: certificate,
                    intermediates: intermediates,
                    server_config: server_config.clone(),
                })
            },
        )
    }
}
