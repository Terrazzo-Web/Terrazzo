use std::sync::Arc;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use openssl::x509::X509;
use rustls::ServerConfig;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::PrivateKeyDer;
use rustls::server::ClientHello;
use rustls::server::ResolvesServerCert;
use rustls::sign::CertifiedKey;
use tracing::debug;
use tracing::info;
use tracing::info_span;
use tracing::warn;

use super::CertificateConfig;
use crate::certificate_info::X509CertificateInfo;
use crate::crypto_provider::crypto_provider;
use crate::security_configuration::certificate::display_x509_certificate;

/// Create a RusTLS [ServerConfig] from a [CertificateConfig].
pub trait ToTlsServer: CertificateConfig + Sized {
    fn to_tls_server(self) -> Result<Arc<ServerConfig>, ToTlsServerError<Self::Error>> {
        to_tls_server_impl(self)
    }
}

impl<T: CertificateConfig> ToTlsServer for T {}

fn to_tls_server_impl<T: CertificateConfig>(
    certificate_config: T,
) -> Result<Arc<ServerConfig>, ToTlsServerError<T::Error>> {
    let _span = info_span!("Setup TLS server certificate").entered();
    let server_config = ServerConfig::builder().with_no_client_auth();
    let mut server_config = if certificate_config.is_dynamic() {
        server_config.with_cert_resolver(Arc::new(ServerCertificateResolver {
            state: Default::default(),
            certificate_config,
        }))
    } else {
        let (certificate_chain, private_key) = build_single_cert::<T>(
            &*certificate_config
                .certificate()
                .map_err(ToTlsServerError::Certificate)?,
            &certificate_config
                .intermediates()
                .map_err(ToTlsServerError::Intermediates)?,
        )?;
        server_config
            .with_single_cert(certificate_chain, private_key)
            .map_err(ToTlsServerError::ServerConfig)?
    };
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(Arc::new(server_config))
}

fn build_single_cert<T: CertificateConfig>(
    certificate: &X509CertificateInfo,
    intermediates: &[X509],
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), ToTlsServerError<T::Error>> {
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
            "Intermediate certificate details: {}",
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

    Ok((certificate_chain, private_key))
}

struct ServerCertificateResolver<T> {
    certificate_config: T,
    state: std::sync::Mutex<Option<CertResolverState>>,
}

struct CertResolverState {
    certified_key: Arc<CertifiedKey>,
    certificate: Arc<X509CertificateInfo>,
    intermediates: Arc<Vec<X509>>,
}

impl<T> std::fmt::Debug for ServerCertificateResolver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertResolver").finish()
    }
}

impl<T: CertificateConfig> ResolvesServerCert for ServerCertificateResolver<T> {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let _span = info_span!(
            "Resolve server certificate",
            host = client_hello.server_name()
        );
        let mut state = self.state.lock().unwrap();
        match self.resolve_impl(&mut state) {
            Ok(certified_key) => Some(certified_key),
            Err(error) => {
                warn!("Failed to resolve server certificate: {error}");
                if let Some(state) = &*state {
                    info!("Reuse stale cached server certificate");
                    Some(state.certified_key.clone())
                } else {
                    None
                }
            }
        }
    }
}

impl<T: CertificateConfig> ServerCertificateResolver<T> {
    fn resolve_impl(
        &self,
        state: &mut Option<CertResolverState>,
    ) -> Result<Arc<CertifiedKey>, ToTlsServerError<T::Error>> {
        let certificate = self
            .certificate_config
            .certificate()
            .map_err(ToTlsServerError::Certificate)?;
        let intermediates = self
            .certificate_config
            .intermediates()
            .map_err(ToTlsServerError::Intermediates)?;

        if let Some(state) = state {
            if Arc::ptr_eq(&certificate, &state.certificate)
                && Arc::ptr_eq(&intermediates, &state.intermediates)
            {
                info!("Reuse cached server certificate");
                return Ok(state.certified_key.clone());
            }
        }

        info!(
            "Use server certificate {:?}",
            certificate.certificate.subject_name()
        );
        debug!("Server certificate {}", certificate.display());
        let certified_key = self.make_certified_key(&certificate, &intermediates)?;
        *state = Some(CertResolverState {
            certified_key: certified_key.clone(),
            certificate,
            intermediates,
        });
        return Ok(certified_key);
    }

    fn make_certified_key(
        &self,
        certificate: &X509CertificateInfo,
        intermediates: &[X509],
    ) -> Result<Arc<CertifiedKey>, ToTlsServerError<T::Error>> {
        let (certificate_chain, private_key) = build_single_cert::<T>(certificate, intermediates)?;
        let certified_key =
            CertifiedKey::from_der(certificate_chain, private_key, crypto_provider())
                .map_err(ToTlsServerError::CertifiedKey)?;
        Ok(Arc::new(certified_key))
    }
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

    #[error("[{n}] {0}", n = self.name())]
    CertifiedKey(rustls::Error),
}
