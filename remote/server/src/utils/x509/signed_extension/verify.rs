use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::x509::store::X509StoreRef;
use tokio_rustls::rustls;
use x509_parser::error::X509Error;
use x509_parser::prelude::FromDer;
use x509_parser::prelude::X509Certificate;
use x509_parser::prelude::X509Extension;

use self::rustls::pki_types::CertificateDer;
use self::signature::verify_signature;
use self::signature::VerifySignatureError;
use self::signer::verify_signer;
use self::signer::VerifySignerError;

mod signature;
mod signer;

pub fn validate_signed_extension(
    certificate: &CertificateDer,
    store: Option<&X509StoreRef>,
    signer_name: &str,
) -> Result<(), ValidateSignedExtensionError> {
    let (_rest, certificate) = X509Certificate::from_der(&certificate)?;
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
        .filter(|extension| extension.oid.to_id_string() == super::SIGNED_EXTENSION_OID)
        .next()
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
