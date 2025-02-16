use std::path::Path;

use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::error::ErrorStack;
use openssl::x509::X509;
use trz_gateway_common::certificate_info::CertificateError;
use trz_gateway_common::certificate_info::CertificateInfo;
use trz_gateway_common::certificate_info::X509CertificateInfo;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificate;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;
use trz_gateway_common::x509::key::make_key;
use trz_gateway_common::x509::key::MakeKeyError;
use trz_gateway_common::x509::PemAsStringError;
use trz_gateway_common::x509::PemString as _;

use crate::certificate_config::ClientCertificateConfig;
use crate::client::certificate::get_certifiate;
use crate::client::certificate::GetCertificateError;
use crate::client::AuthCode;
use crate::http_client::make_http_client;
use crate::http_client::MakeHttpClientError;

/// Default implementation for [TunnelConfig::client_certificate] that loads the
/// certificate and private key and caches them in PEM files.
///
/// [TunnelConfig::client_certificate]: crate::tunnel_config::TunnelConfig::client_certificate
pub async fn load_client_certificate<C: ClientCertificateConfig>(
    client_config: C,
    auth_code: AuthCode,
    certificate_path: CertificateInfo<impl AsRef<Path>>,
) -> Result<PemCertificate, LoadClientCertificateError<C>> {
    let certificate_path = certificate_path.as_ref();
    match certificate_path.map(|path| path.exists()) {
        CertificateInfo {
            certificate: true,
            private_key: true,
        } => {
            let root_ca = certificate_path
                .try_map(std::fs::read_to_string)
                .map_err(LoadClientCertificateError::Load)?;

            Ok(root_ca.into())
        }
        CertificateInfo {
            certificate: false,
            private_key: false,
        } => {
            let client_cert = make_client_certificate(client_config, auth_code).await?;
            let client_cert_pem = CertificateInfo {
                certificate: client_cert.certificate.to_pem(),
                private_key: client_cert.private_key.private_key_to_pem_pkcs8(),
            }
            .try_map(|maybe_pem| maybe_pem.pem_string())?;
            let _: CertificateInfo<()> = certificate_path
                .zip(client_cert_pem.as_ref())
                .try_map(|(path, pem): (&Path, &str)| std::fs::write(path, pem))
                .map_err(LoadClientCertificateError::Store)?;
            Ok(client_cert_pem.into())
        }
        CertificateInfo {
            certificate: root_ca_exists,
            private_key: private_key_exists,
        } => {
            return Err(LoadClientCertificateError::InconsistentState {
                root_ca_exists,
                private_key_exists,
            })
        }
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum LoadClientCertificateError<C: ClientCertificateConfig> {
    #[error("[{n}] Failed to load certificate: {0}", n = self.name())]
    Load(CertificateError<std::io::Error>),

    #[error("[{n}] Failed to load private key: {0}", n = self.name())]
    LoadPrivateKey(std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    Make(#[from] MakeClientCertificateError<C::GatewayPki>),

    #[error("[{n}] Failed to store certificate: {0}", n = self.name())]
    Store(CertificateError<std::io::Error>),

    #[error("[{n}] Inconsistent state: root_ca_exists:{root_ca_exists} private_key_exists:{private_key_exists}", n = self.name())]
    InconsistentState {
        root_ca_exists: bool,
        private_key_exists: bool,
    },

    #[error("[{n}] {0}", n = self.name())]
    PemString(#[from] CertificateError<PemAsStringError>),
}

async fn make_client_certificate<C: ClientCertificateConfig>(
    client_config: C,
    auth_code: AuthCode,
) -> Result<X509CertificateInfo, MakeClientCertificateError<C::GatewayPki>> {
    let key = make_key()?;
    let http_client = make_http_client(client_config.gateway_pki())?;
    let certificate = get_certifiate(client_config, http_client, auth_code, &key).await?;
    let certificate =
        X509::from_pem(certificate.as_bytes()).map_err(MakeClientCertificateError::ParsePem)?;
    Ok(CertificateInfo {
        certificate,
        private_key: key,
    })
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MakeClientCertificateError<C: TrustedStoreConfig> {
    #[error("[{n}] {0}", n = self.name())]
    MakeKey(#[from] MakeKeyError),

    #[error("[{n}] {0}", n = self.name())]
    MakeHttpClient(#[from] MakeHttpClientError<C::Error>),

    #[error("[{n}] {0}", n = self.name())]
    GetCertificate(#[from] GetCertificateError),

    #[error("[{n}] Failed to parse PEM certificate: {0}", n = self.name())]
    ParsePem(ErrorStack),
}
