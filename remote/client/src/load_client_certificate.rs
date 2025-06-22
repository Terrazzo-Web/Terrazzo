//! Utils to load client certificates from PEM files.

use std::path::Path;
use std::time::SystemTime;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use openssl::pkey::PKey;
use openssl::pkey::Private;
use openssl::x509::X509;
use tracing::debug;
use tracing::info;
use tracing::warn;
use trz_gateway_common::certificate_info::CertificateError;
use trz_gateway_common::certificate_info::CertificateInfo;
use trz_gateway_common::certificate_info::X509CertificateInfo;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::security_configuration::certificate::cache::CachedCertificate;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificate;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificateError;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;
use trz_gateway_common::unwrap_infallible::UnwrapInfallible;
use trz_gateway_common::x509::PemAsStringError;
use trz_gateway_common::x509::PemString as _;
use trz_gateway_common::x509::key::MakeKeyError;
use trz_gateway_common::x509::key::make_key;
use trz_gateway_common::x509::time::asn1_to_system_time;

use crate::client::AuthCode;
use crate::client::certificate::GetCertificateError;
use crate::client::certificate::get_certifiate;
use crate::client::config::ClientConfig;
use crate::http_client::MakeHttpClientError;
use crate::http_client::make_http_client;

/// Default implementation for [TunnelConfig::client_certificate] that loads the
/// certificate and private key and caches them in PEM files.
///
/// [TunnelConfig::client_certificate]: crate::tunnel_config::TunnelConfig::client_certificate
pub async fn load_client_certificate<C: ClientConfig>(
    client_config: &C,
    auth_code: AuthCode,
    certificate_path: CertificateInfo<impl AsRef<Path>>,
) -> Result<CachedCertificate, LoadClientCertificateError<C>> {
    let certificate_path = certificate_path.as_ref();
    match certificate_path.map(|path| path.exists()) {
        CertificateInfo {
            certificate: true,
            private_key: true,
        } => {
            info! { "Loading client certificate from {certificate_path:?}" };
            let client_cert_pem = certificate_path
                .try_map(std::fs::read_to_string)
                .map_err(LoadClientCertificateError::Load)?;
            let client_cert = PemCertificate::from(client_cert_pem).cache()?;
            let x509 = client_cert.certificate().unwrap_infallible();
            let expiration =
                asn1_to_system_time(x509.certificate.not_after()).unwrap_or(SystemTime::UNIX_EPOCH);
            if expiration > SystemTime::now() {
                return Ok(client_cert);
            }

            warn!(
                "The client certificate is expired since: {}",
                x509.certificate.not_after()
            );
        }
        CertificateInfo {
            certificate: false,
            private_key: false,
        } => {}
        CertificateInfo {
            certificate: root_ca_exists,
            private_key: private_key_exists,
        } => {
            return Err(LoadClientCertificateError::InconsistentState {
                root_ca_exists,
                private_key_exists,
            });
        }
    }

    info! { "Loading client certificate from {}", client_config.base_url() };
    let client_cert = make_client_certificate(client_config, auth_code).await?;
    let client_cert_pem = store_client_certificate(certificate_path, client_cert)?;
    Ok(PemCertificate::from(client_cert_pem).cache()?)
}

/// Errors returned by [load_client_certificate].
#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum LoadClientCertificateError<C: ClientConfig> {
    #[error("[{n}] Failed to load certificate: {0}", n = self.name())]
    Load(CertificateError<std::io::Error>),

    #[error("[{n}] Failed to load private key: {0}", n = self.name())]
    LoadPrivateKey(std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    Make(#[from] MakeClientCertificateError<C::GatewayPki>),

    #[error("[{n}] {0}", n = self.name())]
    Store(#[from] StoreClientCertificateError),

    #[error("[{n}] Inconsistent state: root_ca_exists:{root_ca_exists} ≠ private_key_exists:{private_key_exists}", n = self.name())]
    InconsistentState {
        root_ca_exists: bool,
        private_key_exists: bool,
    },

    #[error("[{n}] {0}", n = self.name())]
    Pem(#[from] PemCertificateError),
}

pub async fn make_client_certificate<C: ClientConfig>(
    client_config: &C,
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

/// Errors returned by [load_client_certificate] when creating a new
/// certificate, from a new keypair.
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

pub fn store_client_certificate(
    certificate_path: CertificateInfo<&Path>,
    client_cert: CertificateInfo<X509, PKey<Private>>,
) -> Result<CertificateInfo<String>, StoreClientCertificateError> {
    let client_cert_pem = CertificateInfo {
        certificate: client_cert.certificate.to_pem(),
        private_key: client_cert.private_key.private_key_to_pem_pkcs8(),
    }
    .try_map(|maybe_pem| maybe_pem.pem_string())
    .map_err(StoreClientCertificateError::PemString)?;
    let _: CertificateInfo<()> = certificate_path
        .zip(client_cert_pem.as_ref())
        .try_map(|(path, pem): (&Path, &str)| std::fs::write(path, pem))
        .map_err(StoreClientCertificateError::Store)?;
    debug!("Stored client certificate into {certificate_path:?}");
    Ok(client_cert_pem)
}

/// Errors returned by [store_client_certificate].
#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum StoreClientCertificateError {
    #[error("[{n}] Failed to store certificate: {0}", n = self.name())]
    Store(CertificateError<std::io::Error>),

    #[error("[{n}] {0}", n = self.name())]
    PemString(CertificateError<PemAsStringError>),
}
