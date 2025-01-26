use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::cms::CmsContentInfo;
use openssl::error::ErrorStack;
use openssl::x509::store::X509StoreRef;
use x509_parser::prelude::X509Certificate;
use x509_parser::prelude::X509Extension;

use crate::utils::x509::signed_extension::cms_options;
use crate::utils::x509::signed_extension::make_certificate_properties_hash;
use crate::utils::x509::signed_extension::MakeCertificatePropertiesHashError;
use crate::utils::x509::trusted_roots::trusted_roots;
use crate::utils::x509::validity::Validity;

pub fn verify_signature(
    store: Option<&X509StoreRef>,
    certificate: &X509Certificate<'_>,
    signed_extension: &X509Extension<'_>,
) -> Result<(), VerifySignatureError> {
    let mut signed_cms = CmsContentInfo::from_der(signed_extension.value)
        .map_err(VerifySignatureError::CmsContentInfo)?;
    let store = match store {
        Some(store) => store,
        None => trusted_roots(),
    };
    let mut signed_cms_content = vec![];
    let () = signed_cms
        .verify(
            None,
            Some(store),
            None,
            Some(&mut signed_cms_content),
            cms_options(),
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
            .filter(|(_i, (e, a))| e != a)
            .next()
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
