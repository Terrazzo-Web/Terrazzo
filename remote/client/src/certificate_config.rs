use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;

use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::error::ErrorStack;
use openssl::pkey::PKey;
use openssl::pkey::Private;
use openssl::x509::X509;
use reqwest::Certificate;
use tracing::debug;
use trz_gateway_common::id::ClientId;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificate;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;
use trz_gateway_common::x509::key::make_key;
use trz_gateway_common::x509::key::MakeKeyError;
use trz_gateway_common::x509::PemAsStringError;
use trz_gateway_common::x509::PemString as _;
use uuid::Uuid;

use crate::client::certificate::get_certifiate;
use crate::client::certificate::GetCertificateError;
use crate::client::AuthCode;
use crate::client_config::ClientConfig;

/// Configuration to obtain to client certificate.
pub trait ClientCertificateConfig: ClientConfig {
    fn client_id(&self) -> ClientId {
        static CLIENT_ID: OnceLock<ClientId> = OnceLock::new();
        fn make_default_hostname() -> ClientId {
            match hostname::get().map(OsString::into_string) {
                Ok(Ok(hostname)) => return hostname.into(),
                Err(error) => debug!("Failed to get the hostname with hostname::get(): {error}"),
                Ok(Err(error)) => debug!("Failed to parse the hostname string: {error:?}"),
            }
            return Uuid::new_v4().to_string().into();
        }

        CLIENT_ID.get_or_init(make_default_hostname).clone()
    }
}

impl<T: ClientCertificateConfig> ClientCertificateConfig for Arc<T> {}

/// Default implementation for [TunnelConfig::tls] that loads the
/// certificate and private key and caches them in PEM files.
///
/// [TunnelConfig::tls]: crate::tunnel_config::TunnelConfig::tls
pub async fn load_client_certificate<C: ClientCertificateConfig>(
    client_config: C,
    auth_code: AuthCode,
    certificate_path: &Path,
    private_key_path: &Path,
) -> Result<PemCertificate, LoadClientCertificateError<C>> {
    match (certificate_path.exists(), private_key_path.exists()) {
        (true, true) => {
            let root_ca = std::fs::read_to_string(certificate_path)
                .map_err(LoadClientCertificateError::LoadCertificate)?;
            let private_key = std::fs::read_to_string(private_key_path)
                .map_err(LoadClientCertificateError::LoadPrivateKey)?;

            Ok(PemCertificate {
                certificate_pem: root_ca,
                private_key_pem: private_key,
                intermediates_pem: String::default(),
            })
        }
        (false, false) => {
            let (certificate, private_key) =
                make_client_certificate(client_config, auth_code).await?;
            let pem_certificate = PemCertificate {
                certificate_pem: certificate.to_pem().pem_string()?,
                private_key_pem: private_key.private_key_to_pem_pkcs8().pem_string()?,
                intermediates_pem: String::default(),
            };
            std::fs::write(certificate_path, &pem_certificate.certificate_pem)
                .map_err(LoadClientCertificateError::StoreCertificate)?;
            std::fs::write(private_key_path, &pem_certificate.private_key_pem)
                .map_err(LoadClientCertificateError::StorePrivateKey)?;
            Ok(pem_certificate)
        }
        (root_ca_exists, private_key_exists) => {
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
    LoadCertificate(std::io::Error),

    #[error("[{n}] Failed to load private key: {0}", n = self.name())]
    LoadPrivateKey(std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    MakeClientCertificate(#[from] MakeClientCertificateError<C::GatewayPkiConfig>),

    #[error("[{n}] Failed to store certificate: {0}", n = self.name())]
    ParsePem(ErrorStack),

    #[error("[{n}] Failed to store certificate: {0}", n = self.name())]
    StoreCertificate(std::io::Error),

    #[error("[{n}] Failed to store private key: {0}", n = self.name())]
    StorePrivateKey(std::io::Error),

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
) -> Result<(X509, PKey<Private>), MakeClientCertificateError<C::GatewayPkiConfig>> {
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
    GetCertificate(#[from] GetCertificateError),

    #[error("[{n}] Failed to parse PEM certificate: {0}", n = self.name())]
    ParsePem(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    MakeHttpClient(#[from] MakeHttpClientError<C::Error>),
}

fn make_http_client<C>(gateway_pki: C) -> Result<reqwest::Client, MakeHttpClientError<C::Error>>
where
    C: TrustedStoreConfig,
{
    let mut builder = reqwest::Client::builder();
    let roots = gateway_pki
        .root_certificates()
        .map_err(MakeHttpClientError::RootCertificates)?;
    for root in roots.all_certificates() {
        let root_der = root.to_der().map_err(MakeHttpClientError::RootToDer)?;
        let root_certificate =
            Certificate::from_der(&root_der).map_err(MakeHttpClientError::DerToCertificate)?;
        builder = builder.add_root_certificate(root_certificate);
    }
    builder.build().map_err(MakeHttpClientError::Build)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MakeHttpClientError<E: std::error::Error> {
    #[error("[{n}] {0}", n = self.name())]
    RootCertificates(E),

    #[error("[{n}] {0}", n = self.name())]
    RootToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    DerToCertificate(reqwest::Error),

    #[error("[{n}] {0}", n = self.name())]
    Build(reqwest::Error),
}
