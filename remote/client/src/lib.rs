use std::path::PathBuf;

use trz_gateway_common::declare_identifier;

pub struct Client {
    client: reqwest::Client,
    base_url: String,
    certificate_store_path: Option<PathBuf>,
}

#[derive(Default)]
pub enum TrustedRoots {
    #[default]
    System,
    Extra(Vec<String>),
    Only(Vec<String>),
}

declare_identifier!(AuthCode);

impl Client {
    pub async fn get_certifiate(&self, auth_code: AuthCode) -> String {
        let public_key = private_key.public_key_to_pem().pem_string()?;
        let request = client
            .get(format!(
                "https://{}:{}/remote/certificate",
                config.host(),
                config.port
            ))
            .header(CONTENT_TYPE, APPLICATION_JSON.as_ref())
            .body(serde_json::to_string(&GetCertificateRequest {
                code: AuthCode::current(),
                public_key,
                name: "Test cert".into(),
            })?);
        Ok(request.send().await?)
    }

    pub fn connect(&self) {
        // Open WS

        // TLS on top of AsyncRead + AsyncWrite stream

        // Run gRPC server
    }
}
