use std::sync::Arc;

use axum::http::StatusCode;
use axum::Json;
use nameth::nameth;
use nameth::NamedEnumValues as _;
use nameth::NamedType as _;
use serde::Deserialize;
use serde::Serialize;

use super::gateway_configuration::GatewayConfig;
use super::ClientId;
use super::Server;
use crate::auth_code::AuthCode;
use crate::utils::http_error::HttpError;
use crate::utils::http_error::IsHttpError;
use crate::utils::x509::cert::make_cert;
use crate::utils::x509::cert::MakeCertError;
use crate::utils::x509::name::CertitficateName;
use crate::utils::x509::PemAsStringError;
use crate::utils::x509::PemString as _;

#[derive(Debug, Serialize, Deserialize)]
pub struct GetCertificateRequest {
    pub code: AuthCode,
    pub public_key: String,
    pub name: ClientId,
}

impl<C: GatewayConfig> Server<C> {
    pub async fn get_certificate(
        self: Arc<Self>,
        Json(request): Json<GetCertificateRequest>,
    ) -> Result<String, HttpError<GetCertificateError>> {
        if !request.code.is_valid() {
            return Err(GetCertificateError::InvalidAuthCode)?;
        }
        Ok(self
            .make_pem_cert(request)
            .map_err(GetCertificateError::MakeCert)?)
    }

    fn make_pem_cert(
        self: Arc<Self>,
        request: GetCertificateRequest,
    ) -> Result<String, MakePemCertificateError> {
        let certificate = make_cert(
            &self.root_ca.certificate,
            &self.root_ca.private_key,
            CertitficateName {
                common_name: Some(&request.name),
                ..CertitficateName::default()
            },
            self.root_ca.certificate.as_ref().try_into().unwrap(),
            &request.public_key,
            vec![],
        )?;
        Ok(certificate.to_pem().pem_string()?)
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum GetCertificateError {
    #[error("[{n}] {t} is invalid", n = self.name(), t = AuthCode::type_name())]
    InvalidAuthCode,

    #[error("[{n}] {0}", n = self.name())]
    MakeCert(#[from] MakePemCertificateError),
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MakePemCertificateError {
    #[error("[{n}] {0}", n = self.name())]
    MakeCert(#[from] MakeCertError),

    #[error("[{n}] Failed to convert certificate to PEM: {0}", n = self.name())]
    PemString(#[from] PemAsStringError),
}

impl IsHttpError for GetCertificateError {
    fn status_code(&self) -> StatusCode {
        match self {
            GetCertificateError::InvalidAuthCode => StatusCode::FORBIDDEN,
            GetCertificateError::MakeCert(error) => error.status_code(),
        }
    }
}

impl IsHttpError for MakePemCertificateError {
    fn status_code(&self) -> StatusCode {
        match self {
            MakePemCertificateError::MakeCert(error) => error.status_code(),
            MakePemCertificateError::PemString { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
