//! Client certificates from the Terrazzo Gateway.

use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::pkey::HasPublic;
use openssl::pkey::PKeyRef;
use reqwest::StatusCode;
use trz_gateway_common::x509::PemAsStringError;

use super::AuthCode;
use super::config::ClientConfig;
use super::config::SniOverrideError;
use crate::http_client::HttpClient;
use crate::http_client::HttpRequestError;

/// API to obtain client certificates from the Terrazzo Gateway.
pub(crate) async fn get_certifiate(
    client_config: &impl ClientConfig,
    http_client: HttpClient,
    auth_code: AuthCode,
    key: &PKeyRef<impl HasPublic>,
) -> Result<String, GetCertificateError> {
    certificate_request::Run {
        client_config,
        http_client,
        auth_code,
        key,
    }
    .run()
    .await
}

#[autoclone::graph]
mod certificate_request {
    use mime::APPLICATION_JSON;
    use openssl::pkey::HasPublic;
    use openssl::pkey::PKeyRef;
    use reqwest::Url;
    use trz_gateway_common::api::tunnel::GetCertificateRequest;
    use trz_gateway_common::x509::PemAsStringError;
    use trz_gateway_common::x509::PemString as _;

    use super::GetCertificateError;
    use crate::client::AuthCode;
    use crate::client::config::ClientConfig;
    use crate::client::config::SniOverrideError;
    use crate::client::config::set_gateway_sni_override;
    use crate::http_client::HttpClient;

    fn public_key(key: &PKeyRef<impl HasPublic>) -> Result<String, PemAsStringError> {
        key.public_key_to_pem().pem_string()
    }

    fn url(client_config: &impl ClientConfig) -> Result<Url, SniOverrideError> {
        let mut url = client_config.url("/remote/certificate")?;
        set_gateway_sni_override(&mut url, client_config.gateway_sni_override())?;
        Ok(url)
    }

    fn request_body(
        public_key: String,
        auth_code: AuthCode,
        client_config: &impl ClientConfig,
    ) -> Result<String, serde_json::Error> {
        serde_json::to_string(&GetCertificateRequest {
            auth_code,
            public_key,
            name: client_config.client_name(),
        })
    }

    async fn response_body(
        url: Url,
        request_body: String,
        http_client: HttpClient,
    ) -> Result<String, GetCertificateError> {
        let response = http_client
            .get(url, APPLICATION_JSON.as_ref(), request_body)
            .await?;
        let status = response.status;
        let body = response.body;
        if !status.is_success() {
            return Err(GetCertificateError::HttpStatus { status, body });
        }
        Ok(body)
    }

    pub fn run(response_body: String) -> Result<String, GetCertificateError> {
        Ok(response_body)
    }
}

/// Errors returned by [get_certifiate].
#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum GetCertificateError {
    #[error("[{n}] {0}", n = self.name())]
    SniOverride(#[from] SniOverrideError),

    #[error("[{n}] {0}", n = self.name())]
    PublicKeyToPem(#[from] PemAsStringError),

    #[error("[{n}] {0}", n = self.name())]
    RequestSerialization(#[from] serde_json::Error),

    #[error("[{n}] {0}", n = self.name())]
    HttpRequest(#[from] HttpRequestError),

    #[error("[{n}] Gateway returned {status}: {body}", n = self.name())]
    HttpStatus { status: StatusCode, body: String },
}
