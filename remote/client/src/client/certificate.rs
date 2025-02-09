use mime::APPLICATION_JSON;
use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::pkey::HasPublic;
use openssl::pkey::PKeyRef;
use reqwest::header::CONTENT_TYPE;
use trz_gateway_common::api::tunnel::GetCertificateRequest;
use trz_gateway_common::x509::PemAsStringError;
use trz_gateway_common::x509::PemString as _;

use super::AuthCode;
use crate::client_configuration::ClientConfig;

pub trait GetCertifiate: ClientConfig + Sized {
    fn get_certifiate(
        &self,
        auth_code: AuthCode,
        key: &PKeyRef<impl HasPublic>,
    ) -> impl std::future::Future<Output = Result<String, GetCertificateError>> {
        get_certifiate(self, auth_code, key)
    }
}

async fn get_certifiate<T: GetCertifiate>(
    client: &T,
    auth_code: AuthCode,
    key: &PKeyRef<impl HasPublic>,
) -> Result<String, GetCertificateError> {
    let public_key = key.public_key_to_pem().pem_string()?;
    let request = client
        .http_client()
        .get(format!("{}/remote/certificate", client.base_url()))
        .header(CONTENT_TYPE, APPLICATION_JSON.as_ref())
        .body(serde_json::to_string(&GetCertificateRequest {
            auth_code,
            public_key,
            name: "Test cert".into(),
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
