use axum::http::StatusCode;
use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::error::ErrorStack;
use openssl::x509::extension::AuthorityKeyIdentifier;
use openssl::x509::extension::SubjectKeyIdentifier;
use openssl::x509::X509Builder;
use openssl::x509::X509NameRef;
use openssl::x509::X509Ref;
use x509_parser::x509::X509Version;

use super::serial_number::set_serial_number;
use super::serial_number::SetSerialNumberError;
use super::time::SystemToAsn1TimeError;
use super::validity::set_validity;
use super::validity::Validity;
use super::validity::ValidityError;
use crate::http_error::IsHttpError;

pub fn set_common_fields(
    builder: &mut X509Builder,
    issuer_name: &X509NameRef,
    subject_name: &X509NameRef,
    validity: Validity,
) -> Result<(), SetCommonFieldsError> {
    builder
        .set_version(X509Version::V3.0 as i32)
        .map_err(SetCommonFieldsError::SetVersion)?;
    builder
        .set_subject_name(subject_name)
        .map_err(SetCommonFieldsError::SetSubject)?;
    builder
        .set_issuer_name(issuer_name)
        .map_err(SetCommonFieldsError::SetIssuer)?;
    set_validity(builder, validity.try_into()?)?;
    set_serial_number(builder)?;

    (|| {
        let skid = SubjectKeyIdentifier::new().build(&builder.x509v3_context(None, None))?;
        builder.append_extension(skid)?;
        Ok(())
    })()
    .map_err(SetCommonFieldsError::SubjectKeyIdentifier)?;

    Ok(())
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum SetCommonFieldsError {
    #[error("[{n}] Failed to set the X509 version: {0}", n = self.name())]
    SetVersion(ErrorStack),

    #[error("[{n}] Failed to set the subject name: {0}", n = self.name())]
    SetSubject(ErrorStack),

    #[error("[{n}] Failed to set the issuer name: {0}", n = self.name())]
    SetIssuer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    ConvertValidity(#[from] ValidityError<SystemToAsn1TimeError>),

    #[error("[{n}] {0}", n = self.name())]
    SetValidity(#[from] ValidityError<ErrorStack>),

    #[error("[{n}] {0}", n = self.name())]
    SetSerialNumber(#[from] SetSerialNumberError),

    #[error("[{n}] Failed to set SKID: {0}", n = self.name())]
    SubjectKeyIdentifier(ErrorStack),
}

impl IsHttpError for SetCommonFieldsError {
    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

pub(super) fn set_akid(issuer: &X509Ref, builder: &mut X509Builder) -> Result<(), ErrorStack> {
    let akid = AuthorityKeyIdentifier::new()
        .issuer(true)
        .keyid(true)
        .build(&builder.x509v3_context(Some(issuer), None))?;
    builder.append_extension(akid)?;
    Ok(())
}
