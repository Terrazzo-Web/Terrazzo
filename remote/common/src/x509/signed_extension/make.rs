use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::asn1::Asn1OctetString;
use openssl::cms::CmsContentInfo;
use openssl::error::ErrorStack;
use openssl::stack::StackRef;
use openssl::x509::X509;
use openssl::x509::X509Extension;

use super::MakeCertificatePropertiesHashError;
use super::cms_options;
use super::make_certificate_properties_hash;
use super::signed_extension_oid;
use crate::certificate_info::X509CertificateInfoRef;
use crate::x509::validity::Validity;

/// Creates a custom [X509Extension] signed by our issuer.
pub fn make_signed_extension(
    common_name: &str,
    validity: Validity,
    public_key_der: &[u8],
    intermediate_certificates: Option<&StackRef<X509>>,
    signer: X509CertificateInfoRef,
) -> Result<X509Extension, MakeSignedExtensionError> {
    let certificate_properties_hash =
        make_certificate_properties_hash(common_name, validity, public_key_der)?;
    let signed_cms = CmsContentInfo::sign(
        Some(signer.certificate),
        Some(signer.private_key),
        intermediate_certificates,
        Some(&certificate_properties_hash),
        cms_options(),
    )
    .map_err(MakeSignedExtensionError::CmsSign)?;
    let signed_cms_der = signed_cms
        .to_der()
        .map_err(MakeSignedExtensionError::CmsToDer)?;
    let der_encoded_extension = Asn1OctetString::new_from_bytes(&signed_cms_der)
        .map_err(MakeSignedExtensionError::Asn1OctetString)?;
    let x509_extension =
        X509Extension::new_from_der(signed_extension_oid(), false, &der_encoded_extension)
            .map_err(MakeSignedExtensionError::X509Extension)?;
    Ok(x509_extension)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MakeSignedExtensionError {
    #[error("[{n}] {0}", n = self.name())]
    CertificatePropertiesHash(#[from] MakeCertificatePropertiesHashError),

    #[error("[{n}] Failed sign the certificate properties hash: {0}", n = self.name())]
    CmsSign(ErrorStack),

    #[error("[{n}] Failed convert signed CMS to DER: {0}", n = self.name())]
    CmsToDer(ErrorStack),

    #[error("[{n}] Failed to convert the signed CMS DER into a Asn1OctetString: {0}", n = self.name())]
    Asn1OctetString(ErrorStack),

    #[error("[{n}] Failed to make the X509Extension: {0}", n = self.name())]
    X509Extension(ErrorStack),
}
