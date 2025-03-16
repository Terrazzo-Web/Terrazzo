use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use rustls::ServerConfig;
use tracing::debug;

use super::CertificateConfig;
use crate::security_configuration::certificate::display_x509_certificate;

/// Create a RusTLS [ServerConfig] from a [CertificateConfig].
pub trait ToTlsServer: CertificateConfig {
    fn to_tls_server(
        &self,
    ) -> impl Future<Output = Result<ServerConfig, ToTlsServerError<Self::Error>>> {
        to_tls_server_impl(self)
    }
}

impl<T: CertificateConfig> ToTlsServer for T {}

async fn to_tls_server_impl<T: CertificateConfig + ?Sized>(
    certificate_config: &T,
) -> Result<ServerConfig, ToTlsServerError<T::Error>> {
    let certificate = certificate_config
        .certificate()
        .map_err(ToTlsServerError::Certificate)?;

    let mut certificate_chain = vec![];
    {
        debug!("Add leaf certificate: {}", certificate.display());
        let certificate = certificate.certificate.to_der();
        let certificate = certificate.map_err(ToTlsServerError::CertificateToDer)?;
        certificate_chain.push(certificate.into());
    }
    for intermediate in certificate_config
        .intermediates()
        .map_err(ToTlsServerError::Intermediates)?
        .iter()
    {
        debug!(
            "Add intermediate: {}",
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
        .with_single_cert(certificate_chain, private_key)
        .map_err(ToTlsServerError::ServerConfig)?;
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(server_config)
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
