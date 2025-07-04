use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use axum::Json;
use axum::http::StatusCode;
use nameth::NamedEnumValues as _;
use nameth::NamedType as _;
use nameth::nameth;
use openssl::x509::X509Extension;
use pem::PemError;
use tracing::debug;
use tracing::info;
use trz_gateway_common::api::tunnel::GetCertificateRequest;
use trz_gateway_common::http_error::HttpError;
use trz_gateway_common::http_error::IsHttpError;
use trz_gateway_common::is_global::IsGlobalError;
use trz_gateway_common::x509::PemAsStringError;
use trz_gateway_common::x509::PemString as _;
use trz_gateway_common::x509::cert::MakeCertError;
use trz_gateway_common::x509::cert::make_cert;
use trz_gateway_common::x509::name::CertitficateName;
use trz_gateway_common::x509::signed_extension::MakeSignedExtensionError;
use trz_gateway_common::x509::signed_extension::make_signed_extension;
use trz_gateway_common::x509::time::Asn1ToSystemTimeError;
use trz_gateway_common::x509::validity::Validity;
use trz_gateway_common::x509::validity::ValidityError;

use super::IssuerConfig;
use super::Server;
use crate::auth_code::AuthCode;

static CERTIFICATE_VALIDITY: Duration = Duration::from_secs(3600 * 24 * 90);

impl Server {
    /// API to issue client certificates.
    ///
    /// Endpoint: "/remote/certificate"
    pub async fn get_certificate(
        self: Arc<Self>,
        Json(request): Json<GetCertificateRequest<AuthCode>>,
    ) -> Result<String, HttpError<GetCertificateError>> {
        if !request.auth_code.is_valid() {
            info!(
                "Invalid auth code. Got '{}' expected '{}'",
                request.auth_code,
                AuthCode::current()
            );
            return Err(GetCertificateError::InvalidAuthCode)?;
        }
        Ok(self.make_pem_cert(request)?)
    }

    fn make_pem_cert(
        &self,
        request: GetCertificateRequest<AuthCode>,
    ) -> Result<String, GetCertificateError> {
        let issuer_config = self
            .issuer_config
            .get()
            .map_err(GetCertificateError::IssuerConfig)?;
        let now = SystemTime::now();
        let mut validity = issuer_config.validity;
        validity.from = now;
        validity.to = SystemTime::min(validity.to, now + CERTIFICATE_VALIDITY);
        debug!(
            "Issuing client certificate for {} valid until {:?}",
            request.name, validity.to
        );
        let signed_extension = self.make_signed_extension(&issuer_config, &request, validity)?;
        Ok(self.assemble_pem_cert(request, validity, signed_extension)?)
    }

    fn make_signed_extension(
        &self,
        issuer_config: &IssuerConfig,
        request: &GetCertificateRequest<AuthCode>,
        validity: Validity,
    ) -> Result<X509Extension, GetCertificateError> {
        Ok(make_signed_extension(
            &request.name,
            validity,
            pem::parse(&request.public_key)
                .map_err(GetCertificateError::InvalidPublicKeyPem)?
                .contents(),
            Some(&issuer_config.intermediates),
            (*issuer_config.signer).as_ref(),
        )?)
    }

    fn assemble_pem_cert(
        &self,
        request: GetCertificateRequest<AuthCode>,
        validity: Validity,
        signed_extension: X509Extension,
    ) -> Result<String, MakePemCertificateError> {
        let certificate = make_cert(
            (*self.root_ca).as_ref(),
            CertitficateName {
                common_name: Some(&request.name),
                ..CertitficateName::default()
            },
            validity,
            &request.public_key,
            vec![signed_extension],
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
    Validity(#[from] ValidityError<Asn1ToSystemTimeError>),

    #[error("[{n}] {0}", n = self.name())]
    InvalidPublicKeyPem(PemError),

    #[error("[{n}] {0}", n = self.name())]
    MakeSignedExtension(#[from] MakeSignedExtensionError),

    #[error("[{n}] {0}", n = self.name())]
    MakeCert(#[from] MakePemCertificateError),

    #[error("[{n}] {0}", n = self.name())]
    IssuerConfig(Arc<dyn IsGlobalError>),
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
            Self::InvalidAuthCode => StatusCode::FORBIDDEN,
            Self::MakeCert(error) => error.status_code(),
            Self::Validity { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidPublicKeyPem { .. } => StatusCode::BAD_REQUEST,
            Self::MakeSignedExtension { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::IssuerConfig { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IsHttpError for MakePemCertificateError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::MakeCert(error) => error.status_code(),
            Self::PemString { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
