use mime::APPLICATION_JSON;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::pkey::HasPublic;
use openssl::pkey::PKeyRef;
use reqwest::Client;
use reqwest::header::CONTENT_TYPE;
use trz_gateway_common::api::tunnel::GetCertificateRequest;
use trz_gateway_common::x509::PemAsStringError;
use trz_gateway_common::x509::PemString as _;

use super::AuthCode;
use crate::client_config::ClientConfig;

/// API client to obtain client certificates from the Terrazzo Gateway.
pub async fn get_certifiate(
    client_config: impl ClientConfig,
    http_client: Client,
    auth_code: AuthCode,
    key: &PKeyRef<impl HasPublic>,
) -> Result<String, GetCertificateError> {
    let public_key = key.public_key_to_pem().pem_string()?;
    let request = http_client
        .get(format!("{}/remote/certificate", client_config.base_url()))
        .header(CONTENT_TYPE, APPLICATION_JSON.as_ref())
        .body(serde_json::to_string(&GetCertificateRequest {
            auth_code,
            public_key,
            name: client_config.client_name(),
        })?);
    Ok(request.send().await?.text().await?)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum GetCertificateError {
    #[error("[{n}] {0}", n = self.name())]
    PublicKeyToPem(#[from] PemAsStringError),

    #[error("[{n}] {0}", n = self.name())]
    RequestSerialization(#[from] serde_json::Error),

    #[error("[{n}] {0}", n = self.name())]
    HttpRequest(#[from] reqwest::Error),
}
