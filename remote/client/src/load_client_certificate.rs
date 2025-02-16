use std::path::Path;

use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::error::ErrorStack;
use openssl::pkey::PKey;
use openssl::pkey::Private;
use openssl::x509::X509;
use trz_gateway_common::certificate_info::CertificateError;
use trz_gateway_common::certificate_info::CertificateInfo;
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
    certificate_info: CertificateInfo<&'static Path>,
) -> Result<PemCertificate, LoadClientCertificateError<C>> {
    match certificate_info.map(|path| path.exists()) {
        CertificateInfo {
            certificate: true,
            private_key: true,
        } => {
            let root_ca = certificate_info
                .try_map(std::fs::read_to_string)
                .map_err(LoadClientCertificateError::Load)?;

            Ok(PemCertificate {
                certificate_pem: root_ca.certificate,
                private_key_pem: root_ca.private_key,
                intermediates_pem: String::default(),
            })
        }
        CertificateInfo {
            certificate: false,
            private_key: false,
        } => {
            let (certificate, private_key) =
                make_client_certificate(client_config, auth_code).await?;
            let pem_certificate = PemCertificate {
                certificate_pem: certificate.to_pem().pem_string()?,
                private_key_pem: private_key.private_key_to_pem_pkcs8().pem_string()?,
                intermediates_pem: String::default(),
            };
            let _: CertificateInfo<()> = certificate_info
                .zip(CertificateInfo {
                    certificate: &pem_certificate.certificate_pem,
                    private_key: &pem_certificate.private_key_pem,
                })
                .try_map(|(path, pem)| std::fs::write(path, pem))
                .map_err(LoadClientCertificateError::Store)?;
            Ok(pem_certificate)
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
    PemString(#[from] PemAsStringError),
}

async fn make_client_certificate<C: ClientCertificateConfig>(
    client_config: C,
    auth_code: AuthCode,
) -> Result<(X509, PKey<Private>), MakeClientCertificateError<C::GatewayPki>> {
    let key = make_key()?;
    let http_client = make_http_client(client_config.gateway_pki())?;
    let certificate = get_certifiate(client_config, http_client, auth_code, &key).await?;
    let certificate =
        X509::from_pem(certificate.as_bytes()).map_err(MakeClientCertificateError::ParsePem)?;
    Ok((certificate, key))
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
