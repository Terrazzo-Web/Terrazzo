use std::sync::OnceLock;
use std::time::SystemTimeError;
use std::time::UNIX_EPOCH;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::asn1::Asn1Object;
use openssl::asn1::Asn1ObjectRef;
use openssl::cms::CMSOptions;
use openssl::error::ErrorStack;
use openssl::hash::DigestBytes;
use openssl::hash::Hasher;
use openssl::hash::MessageDigest;

use super::validity::Validity;
use super::validity::ValidityError;

mod make;
mod verify;

pub use self::make::MakeSignedExtensionError;
pub use self::make::make_signed_extension;
pub use self::verify::ValidateSignedExtensionError;
pub use self::verify::validate_signed_extension;

const SIGNED_EXTENSION_OID: &str = "1.3.6.1.4.1.311.10.99.1";

fn signed_extension_oid() -> &'static Asn1ObjectRef {
    static ASN1_OBJECT: OnceLock<Asn1Object> = OnceLock::new();
    ASN1_OBJECT.get_or_init(|| {
        Asn1Object::from_str(SIGNED_EXTENSION_OID)
            .expect("Failed to cast SIGNED_EXTENSION_OID to Asn1Object")
    })
}

fn make_certificate_properties_hash(
    common_name: &str,
    validity: Validity,
    public_key_der: &[u8],
) -> Result<Vec<u8>, MakeCertificatePropertiesHashError> {
    let validity = validity.try_map(|t| t.duration_since(UNIX_EPOCH))?;
    let mut certificate_properties_hash = format!(
        "{common_name}:{}:{}:",
        validity.from.as_secs(),
        validity.to.as_secs(),
    )
    .into_bytes();
    let public_key_sha256 = make_public_key_sha256(public_key_der)
        .map_err(MakeCertificatePropertiesHashError::PublicKeySha256)?;
    certificate_properties_hash.extend(public_key_sha256.as_ref());
    Ok(certificate_properties_hash)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MakeCertificatePropertiesHashError {
    #[error("[{n}] Failed to create validity hash: {0}", n = self.name())]
    Validity(#[from] ValidityError<SystemTimeError>),

    #[error("[{n}] Failed to compute public key's sha256: {0}", n = self.name())]
    PublicKeySha256(ErrorStack),
}

fn make_public_key_sha256(public_key_der: &[u8]) -> Result<DigestBytes, ErrorStack> {
    let mut hasher = Hasher::new(MessageDigest::sha256())?;
    hasher.update(public_key_der)?;
    hasher.finish()
}

fn cms_options() -> CMSOptions {
    CMSOptions::BINARY | CMSOptions::USE_KEYID
}
