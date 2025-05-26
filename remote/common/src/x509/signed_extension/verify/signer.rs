use cms::cert::CertificateChoices;
use cms::cert::x509::certificate::CertificateInner;
use cms::cert::x509::der::Decode;
use cms::cert::x509::der::Encode;
use cms::cert::x509::der::asn1::PrintableString;
use cms::cert::x509::ext::pkix::SubjectKeyIdentifier;
use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use cms::signed_data::SignerIdentifier;
use cms::signed_data::SignerInfo;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use oid_registry::OID_PKCS7_ID_SIGNED_DATA;
use oid_registry::OID_X509_COMMON_NAME;
use oid_registry::OID_X509_EXT_SUBJECT_KEY_IDENTIFIER;
use openssl::error::ErrorStack;
use openssl::x509::X509StoreContext;
use tracing::debug;
use tracing::error;
use x509_parser::prelude::X509Extension;

use crate::security_configuration::trusted_store::TrustedStoreConfig;
use crate::x509::stack::make_stack;

/// Validates the custom X509 extension is signed by our issuer.
///
/// The validation of the issuer happens in [super::signature::verify_signature]
pub fn verify_signer(
    store: &impl TrustedStoreConfig,
    signer_name: &str,
    signed_extension: &X509Extension<'_>,
) -> Result<(), VerifySignerError> {
    let signed_data = ContentInfo::from_der(signed_extension.value)
        .map_err(|_| VerifySignerError::NotSignedCms)?;
    if signed_data.content_type.as_bytes() != OID_PKCS7_ID_SIGNED_DATA.as_bytes() {
        return Err(VerifySignerError::NotSignedCms);
    }
    let signed_data = signed_data.content.decode_as();
    let signed_data: SignedData = signed_data.map_err(|_| VerifySignerError::NotSignedCms)?;
    let signer_info = get_signer_info(&signed_data)?;
    let signing_certificate = find_signing_certificate(store, signer_info, &signed_data)?;
    if signer_name != signing_certificate {
        return Err(VerifySignerError::SignerCertificateNameMismatch(
            signing_certificate,
        ));
    }
    Ok(())
}

fn get_signer_info(signed_data: &SignedData) -> Result<&SignerInfo, VerifySignerError> {
    let mut single_signer_info = None;
    for signer_info in signed_data.signer_infos.0.iter() {
        if single_signer_info.is_some() {
            return Err(VerifySignerError::MultipleSignerInfos);
        }
        single_signer_info = Some(signer_info);
    }
    return single_signer_info.ok_or(VerifySignerError::MissingSignerInfo);
}

fn find_signing_certificate(
    store: &impl TrustedStoreConfig,
    signer_info: &SignerInfo,
    signed_data: &SignedData,
) -> Result<String, VerifySignerError> {
    let SignerIdentifier::SubjectKeyIdentifier(signer_skid) = &signer_info.sid else {
        return Err(VerifySignerError::SignerIdentifierNotSupported);
    };

    let certificates = signed_data
        .certificates
        .as_ref()
        .ok_or(VerifySignerError::MissingSignerCertificate)?;
    for certificate in certificates.0.iter() {
        let CertificateChoices::Certificate(certificate) = certificate else {
            continue;
        };

        {
            let Some(extensions) = &certificate.tbs_certificate.extensions else {
                continue;
            };
            let Some(skid_extension) = extensions.iter().find(|extension| {
                extension.extn_id.as_bytes() == OID_X509_EXT_SUBJECT_KEY_IDENTIFIER.as_bytes()
            }) else {
                continue;
            };
            let skid_extension = skid_extension.extn_value.as_bytes();
            let Ok(certificate_skid) = SubjectKeyIdentifier::from_der(skid_extension) else {
                continue;
            };
            if signer_skid.0.as_bytes() != certificate_skid.0.as_bytes() {
                continue;
            }
        }

        let subject = &certificate.tbs_certificate.subject;
        let mut distinguished_names = subject.0.iter().flat_map(|dn| dn.0.iter());
        let Some(common_name) = distinguished_names
            .find(|entry| entry.oid.as_bytes() == OID_X509_COMMON_NAME.as_bytes())
        else {
            continue;
        };

        let () = verify_signer_chain(store, signed_data, certificate).map_err(Box::new)?;

        return common_name
            .value
            .decode_as::<String>()
            .or_else(|_| {
                common_name
                    .value
                    .decode_as::<PrintableString>()
                    .map(|s| s.to_string())
            })
            .map_err(|error| {
                error!("Failed to parse {common_name:?}: {error}");
                VerifySignerError::SignerCertificateNameError
            });
    }
    Err(VerifySignerError::MissingSignerCertificate)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum VerifySignerError {
    #[error("[{n}] The signed extension didn't contain a Signed CMS", n = self.name())]
    NotSignedCms,

    #[error("[{n}] The signed CMS contains multiple signers", n = self.name())]
    MultipleSignerInfos,

    #[error("[{n}] The signed CMS doesn't have any signers", n = self.name())]
    MissingSignerInfo,

    #[error("[{n}] We only support SubjectKeyIdentifier", n = self.name())]
    SignerIdentifierNotSupported,

    #[error("[{n}] The signer certificate was not found", n = self.name())]
    MissingSignerCertificate,

    #[error("[{n}] The signer certificate name was not found", n = self.name())]
    SignerCertificateNameError,

    #[error("[{n}] The signer certificate name was: {0}", n = self.name())]
    SignerCertificateNameMismatch(String),

    #[error("[{n}] {0}", n = self.name())]
    VerifySignerChainError(#[from] Box<VerifySignerChainError>),
}

fn verify_signer_chain(
    store: &impl TrustedStoreConfig,
    signed_data: &SignedData,
    certificate: &CertificateInner,
) -> Result<(), VerifySignerChainError> {
    let store = store
        .root_certificates()
        .map_err(|error| VerifySignerChainError::RootCertificates(error.into()))?;

    let leaf = {
        let der = certificate
            .to_der()
            .map_err(|_| VerifySignerChainError::Leaf)?;
        openssl::x509::X509::from_der(&der).map_err(|_| VerifySignerChainError::Leaf)?
    };
    debug!("Validating leaf certificate: {:?}", leaf.subject_name());

    let chain = make_stack(
        signed_data
            .certificates
            .iter()
            .flat_map(|certificate| certificate.0.iter())
            .flat_map(|certificate| match certificate {
                CertificateChoices::Certificate(certificate) => Some(certificate),
                CertificateChoices::Other { .. } => None,
            })
            .flat_map(|certificate| {
                let der = certificate.to_der().ok()?;
                openssl::x509::X509::from_der(&der).ok()
            })
            .inspect(|certificate| debug!("Building chain with: {:?}", certificate.subject_name())),
    )
    .map_err(|_| VerifySignerChainError::Chain)?;

    let mut ctx = X509StoreContext::new().map_err(|_| VerifySignerChainError::Ctx)?;
    ctx.init(&store, &leaf, &chain, |c| c.verify_cert())
        .map_err(VerifySignerChainError::SignerCertificateInvalid)?
        .then_some(())
        .ok_or(VerifySignerChainError::ValidationFailed)?;

    debug!("Validation PASSED");
    Ok(())
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum VerifySignerChainError {
    #[error("[{n}] Failed to load root certificates", n = self.name())]
    RootCertificates(Box<dyn std::error::Error>),

    #[error("[{n}] Can't parse the signer certificate", n = self.name())]
    Leaf,

    #[error("[{n}] Can't build the certificate chain", n = self.name())]
    Chain,

    #[error("[{n}] Can't build the X509 ctx", n = self.name())]
    Ctx,

    #[error("[{n}] The chain is invalid {0}", n = self.name())]
    SignerCertificateInvalid(ErrorStack),

    #[error("[{n}] The signer certificate is invalid", n = self.name())]
    ValidationFailed,
}
