use nameth::NamedEnumValues as _;
use nameth::nameth;
use rustls::pki_types::CertificateDer;
use x509_parser::error::X509Error;
use x509_parser::prelude::FromDer;
use x509_parser::prelude::X509Certificate;
use x509_parser::prelude::X509Extension;

use self::signature::VerifySignatureError;
use self::signature::verify_signature;
use self::signer::VerifySignerError;
use self::signer::verify_signer;
use crate::security_configuration::trusted_store::TrustedStoreConfig;

mod signature;
mod signer;

/// Validates the certifcate has a custom extension containing a CMS signed by
/// our issuer certificate.
pub fn validate_signed_extension(
    certificate: &CertificateDer,
    store: &impl TrustedStoreConfig,
    signer_name: &str,
) -> Result<(), ValidateSignedExtensionError> {
    let (_rest, certificate) = X509Certificate::from_der(certificate)?;
    let signed_extension = find_signed_extension(&certificate)?;
    let () = verify_signer(signer_name, signed_extension)?;
    let () = verify_signature(store, &certificate, signed_extension)?;
    Ok(())
}

fn find_signed_extension<'t>(
    certificate: &'t X509Certificate<'t>,
) -> Result<&'t X509Extension<'t>, ValidateSignedExtensionError> {
    certificate
        .extensions()
        .iter()
        .find(|extension| extension.oid.to_id_string() == super::SIGNED_EXTENSION_OID)
        .ok_or(ValidateSignedExtensionError::SignedExtensionNotFound)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ValidateSignedExtensionError {
    #[error("[{n}] Failed to parse X509Certificate: {0}", n = self.name())]
    X509Certificate(#[from] x509_parser::nom::Err<X509Error>),

    #[error("[{n}] The certificate didn't have a signed extension", n = self.name())]
    SignedExtensionNotFound,

    #[error("[{n}] {0}", n = self.name())]
    VerifySignature(#[from] VerifySignatureError),

    #[error("[{n}] {0}", n = self.name())]
    VerifySigner(#[from] VerifySignerError),
}

impl From<ValidateSignedExtensionError> for rustls::Error {
    fn from(error: ValidateSignedExtensionError) -> Self {
        rustls::Error::InvalidCertificate(match error {
            ValidateSignedExtensionError::X509Certificate { .. } => {
                rustls::CertificateError::BadEncoding
            }
            ValidateSignedExtensionError::SignedExtensionNotFound { .. }
            | ValidateSignedExtensionError::VerifySignature { .. }
            | ValidateSignedExtensionError::VerifySigner { .. } => {
                rustls::CertificateError::ApplicationVerificationFailure
            }
        })
    }
}
