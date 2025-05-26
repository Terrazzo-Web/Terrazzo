use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::cms::CMSOptions;
use openssl::cms::CmsContentInfo;
use openssl::error::ErrorStack;
use x509_parser::prelude::X509Certificate;
use x509_parser::prelude::X509Extension;

use crate::security_configuration::trusted_store::TrustedStoreConfig;
use crate::x509::signed_extension::MakeCertificatePropertiesHashError;
use crate::x509::signed_extension::cms_options;
use crate::x509::signed_extension::make_certificate_properties_hash;
use crate::x509::validity::Validity;

/// Validates the custom X509 extension matches the client certificate.
pub fn verify_signature(
    store: &impl TrustedStoreConfig,
    certificate: &X509Certificate<'_>,
    signed_extension: &X509Extension<'_>,
) -> Result<(), VerifySignatureError> {
    let store = Some(
        store
            .root_certificates()
            .map_err(|error| VerifySignatureError::RootCertificates(error.into()))?,
    );
    let store = store.as_ref().map(|s| s.as_ref().as_ref());
    let mut signed_cms = CmsContentInfo::from_der(signed_extension.value)
        .map_err(VerifySignatureError::CmsContentInfo)?;
    let mut signed_cms_content = vec![];
    let () = signed_cms
        .verify(
            None,
            store,
            None,
            Some(&mut signed_cms_content),
            cms_options() | CMSOptions::NO_SIGNER_CERT_VERIFY,
        )
        .map_err(VerifySignatureError::SignedCmsInvalid)?;

    let certificate_properties_hash = {
        let validity = certificate.validity();
        let validity = Validity {
            from: validity.not_before.to_datetime().into(),
            to: validity.not_after.to_datetime().into(),
        };
        let public_key_der = certificate.public_key().raw;
        let common_name = certificate
            .subject()
            .iter_common_name()
            .map(|common_name| common_name.as_str())
            .next()
            .unwrap_or(Ok("Not found !!!"))
            .unwrap_or("Invalid UTF-8 !!!");
        make_certificate_properties_hash(common_name, validity, public_key_der)?
    };

    if certificate_properties_hash == signed_cms_content {
        Ok(())
    } else {
        let expected = certificate_properties_hash.split(|c| *c == b':').take(4);
        let actual = signed_cms_content.split(|c| *c == b':').take(4);
        let (field, e, a) = expected
            .zip(actual)
            .enumerate()
            .find(|(_i, (e, a))| e != a)
            .map(|(i, (e, a))| {
                let field = match i {
                    0 => "common_name",
                    1 => "not_before",
                    2 => "not_after",
                    3 => "public_key",
                    _ => "unknown",
                };
                (field, e, a)
            })
            .unwrap_or(("Diff not found", &[], &[]));
        Err(VerifySignatureError::CertificatePropertiesMismatch {
            field,
            expected: String::from_utf8(e.to_owned()).unwrap_or_else(|_| "Opaque".to_owned()),
            actual: String::from_utf8(a.to_owned()).unwrap_or_else(|_| "Opaque".to_owned()),
        })
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum VerifySignatureError {
    #[error("[{n}] Failed to load root certificates", n = self.name())]
    RootCertificates(Box<dyn std::error::Error>),

    #[error("[{n}] The signed extension didn't contain a Signed CMS", n = self.name())]
    CmsContentInfo(ErrorStack),

    #[error("[{n}] Failed to verify the Signed CMS: {0}", n = self.name())]
    SignedCmsInvalid(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    CertificatePropertiesHash(#[from] MakeCertificatePropertiesHashError),

    #[error("[{n}] The signed extension content hash doesn't match: {field} was '{actual}' expected '{expected}'", n = self.name())]
    CertificatePropertiesMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
}
