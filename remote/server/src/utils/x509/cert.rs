use axum::http::StatusCode;
use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::error::ErrorStack;
use openssl::pkey::PKey;
use openssl::pkey::PKeyRef;
use openssl::pkey::Private;
use openssl::x509::extension::BasicConstraints;
use openssl::x509::extension::ExtendedKeyUsage;
use openssl::x509::extension::KeyUsage;
use openssl::x509::extension::SubjectAlternativeName;
use openssl::x509::X509Builder;
use openssl::x509::X509Extension;
use openssl::x509::X509Ref;
use openssl::x509::X509;

use super::common_fields::set_akid;
use super::common_fields::set_common_fields;
use super::common_fields::SetCommonFieldsError;
use super::name::make_name;
use super::name::CertitficateName;
use super::name::MakeNameError;
use super::validity::Validity;
use crate::utils::http_error::IsHttpError;

pub fn make_cert(
    issuer: &X509Ref,
    issuer_key: &PKeyRef<Private>,
    name: CertitficateName,
    validity: Validity,
    public_key: &str,
    extensions: Vec<X509Extension>,
) -> Result<X509, MakeCertError> {
    let mut builder = X509Builder::new().map_err(MakeCertError::NewBuilder)?;

    let public_key =
        PKey::public_key_from_pem(public_key.as_bytes()).map_err(MakeCertError::ParsePublicKey)?;
    builder
        .set_pubkey(&public_key)
        .map_err(MakeCertError::SetPublicKey)?;

    {
        let name = make_name(name)?;
        set_common_fields(&mut builder, issuer.subject_name(), &name, validity)?;
    }

    (|| {
        let basic_constraints = BasicConstraints::new().critical().build()?;
        builder.append_extension(basic_constraints)?;
        Ok(())
    })()
    .map_err(MakeCertError::BasicConstraints)?;

    (|| {
        let key_usage = KeyUsage::new().critical().digital_signature().build()?;
        builder.append_extension(key_usage)?;
        Ok(())
    })()
    .map_err(MakeCertError::KeyUsage)?;

    (|| {
        let key_usage = ExtendedKeyUsage::new()
            .critical()
            .server_auth()
            .client_auth()
            .email_protection()
            .build()?;
        builder.append_extension(key_usage)?;
        Ok(())
    })()
    .map_err(MakeCertError::ExtendedKeyUsage)?;

    set_akid(issuer, &mut builder).map_err(MakeCertError::AuthorityKeyIdentifier)?;

    if let Some(common_name) = name.common_name {
        (|| {
            builder.append_extension(
                SubjectAlternativeName::new()
                    .dns(common_name)
                    .build(&builder.x509v3_context(Some(issuer), None))?,
            )?;
            Ok(())
        })()
        .map_err(MakeCertError::SubjectAlternativeName)?;
    }

    for extension in extensions {
        builder
            .append_extension(extension)
            .map_err(MakeCertError::AppendCustomExtension)?;
    }

    builder
        .sign(issuer_key, openssl::hash::MessageDigest::sha256())
        .map_err(MakeCertError::Sign)?;

    let certificate = builder.build();

    Ok(certificate)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MakeCertError {
    #[error("[{n}] Failed to create a new X509 Certificate builder: {0}", n = self.name())]
    NewBuilder(ErrorStack),

    #[error("[{n}] Failed to parse PEM public key: {0}", n = self.name())]
    ParsePublicKey(ErrorStack),

    #[error("[{n}] Failed to set the public key: {0}", n = self.name())]
    SetPublicKey(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    MakeName(#[from] MakeNameError),

    #[error("[{n}] {0}", n = self.name())]
    SetCommonFieldsError(#[from] SetCommonFieldsError),

    #[error("[{n}] Failed to set basic constraints: {0}", n = self.name())]
    BasicConstraints(ErrorStack),

    #[error("[{n}] Failed to set key usage: {0}", n = self.name())]
    KeyUsage(ErrorStack),

    #[error("[{n}] Failed to set extended key usage: {0}", n = self.name())]
    ExtendedKeyUsage(ErrorStack),

    #[error("[{n}] Failed to set AKID: {0}", n = self.name())]
    AuthorityKeyIdentifier(ErrorStack),

    #[error("[{n}] Failed to set subject alternative name: {0}", n = self.name())]
    SubjectAlternativeName(ErrorStack),

    #[error("[{n}] Failed add custom extension: {0}", n = self.name())]
    AppendCustomExtension(ErrorStack),

    #[error("[{n}] Failed to sign the certificate: {0}", n = self.name())]
    Sign(ErrorStack),
}

impl IsHttpError for MakeCertError {
    fn status_code(&self) -> StatusCode {
        match self {
            MakeCertError::ParsePublicKey { .. } => StatusCode::BAD_REQUEST,
            MakeCertError::MakeName(error) => error.status_code(),
            MakeCertError::SetCommonFieldsError(error) => error.status_code(),
            MakeCertError::AppendCustomExtension { .. } => StatusCode::BAD_REQUEST,
            MakeCertError::NewBuilder { .. }
            | MakeCertError::SetPublicKey { .. }
            | MakeCertError::BasicConstraints { .. }
            | MakeCertError::KeyUsage { .. }
            | MakeCertError::ExtendedKeyUsage { .. }
            | MakeCertError::AuthorityKeyIdentifier { .. }
            | MakeCertError::SubjectAlternativeName { .. }
            | MakeCertError::Sign { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::time::Duration;
    use std::time::SystemTime;

    use openssl::pkey::PKey;
    use openssl::pkey::PKeyRef;
    use openssl::pkey::Private;
    use openssl::pkey::Public;
    use openssl::sign::Signer;
    use openssl::sign::Verifier;
    use openssl::stack::Stack;
    use openssl::x509::store::X509StoreBuilder;
    use openssl::x509::X509Extension;
    use openssl::x509::X509Ref;
    use openssl::x509::X509;
    use scopeguard::defer_on_unwind;
    use tokio_rustls::rustls::pki_types::CertificateDer;

    use super::super::name::CertitficateName;
    use crate::utils::x509::ca::make_ca;
    use crate::utils::x509::ca::make_intermediate;
    use crate::utils::x509::ca::MakeCaError;
    use crate::utils::x509::key::make_key;
    use crate::utils::x509::signed_extension::make_signed_extension;
    use crate::utils::x509::signed_extension::validate_signed_extension;
    use crate::utils::x509::validity::Validity;
    use crate::utils::x509::PemString as _;

    const DATA: &str = "Hello, world! 😃";

    #[test]
    fn make_cert() -> Result<(), Box<dyn Error>> {
        let (ca, ca_key) = make_test_ca()?;
        Ok({
            let (certificate, _private_key) = make_test_cert(&ca, &ca_key)?;
            let text = certificate.to_text().pem_string()?;
            let ca_text = ca.to_text().pem_string()?;
            let _debug = scopeguard::guard_on_unwind((), |_| {
                println!("CA is\n{ca_text}");
                println!("Certificate is\n{text}");
            });

            assert!(text.contains("Signature Algorithm: ecdsa-with-SHA256"));
            assert!(text.contains(
                "Issuer: C=DE, ST=Bayern, L=Munich, O=Terrazzo, CN=Terrazzo Test Root CA"
            ));
            assert!(
                text.contains("Subject: C=DE, ST=Bayern, L=Munich, O=Terrazzo, CN=Terrazzo Client")
            );
            assert!(text.contains("X509v3 Subject Key Identifier"));
            assert!(text.contains("X509v3 Authority Key Identifier"));
            // AuthorityKeyIdentifier.issuer(true)
            assert!(text
                .contains("DirName:/C=DE/ST=Bayern/L=Munich/O=Terrazzo/CN=Terrazzo Test Root CA"));
            assert!(!text
                .to_ascii_uppercase()
                .contains("DA:39:A3:EE:5E:6B:4B:0D:32:55:BF:EF:95:60:18:90:AF:D8:07:09"));
        })
    }

    #[test]
    fn sign_payload() -> Result<(), Box<dyn Error>> {
        let (ca, ca_key) = make_test_ca()?;

        Ok({
            let (certificate, private_key) = make_test_cert(&ca, &ca_key)?;

            let signature = {
                let mut signer = Signer::new_without_digest(&private_key)?;
                signer.update(DATA.as_bytes())?;
                signer.sign_to_vec()?
            };

            assert!(validate_signature(certificate.public_key()?, &signature)?);

            let (certificate, _private_key) = make_test_cert(&ca, &ca_key)?;
            assert!(!validate_signature(certificate.public_key()?, &signature)?);
        })
    }

    fn validate_signature(
        public_key: PKey<Public>,
        signature: &[u8],
    ) -> Result<bool, Box<dyn Error>> {
        let mut verifier = Verifier::new_without_digest(&public_key)?;
        verifier.update(DATA.as_bytes())?;
        Ok(verifier.verify(&signature)?)
    }

    #[test]
    fn signed_extension() -> Result<(), Box<dyn Error>> {
        let test_case = SignedExtensionTestCase::new()?;
        let extension = make_test_signed_extension(&test_case)?;
        let certificate = make_test_cert_with_signed_extension(&test_case, extension)?;
        validate_test_signed_extension(&test_case, certificate)?;
        Ok(())
    }

    #[test]
    fn signed_extension_wrong_name() -> Result<(), Box<dyn Error>> {
        let test_case = SignedExtensionTestCase::new()?;
        let extension = make_test_signed_extension(&SignedExtensionTestCase {
            common_name: format!("NOT {}", test_case.common_name),
            ..test_case.clone()
        })?;
        let certificate = make_test_cert_with_signed_extension(&test_case, extension)?;
        let error = validate_test_signed_extension(&test_case, certificate).unwrap_err();
        defer_on_unwind!(eprintln!("{error}"));
        assert_eq!(
            error.to_string(),
            "[VerifySignature] [CertificatePropertiesMismatch] The signed extension content hash doesn't match: common_name was 'NOT With signed extension' expected 'With signed extension'"
        );
        Ok(())
    }

    #[test]
    fn signed_extension_wrong_validity_from() -> Result<(), Box<dyn Error>> {
        let test_case = SignedExtensionTestCase::new()?;
        let extension = make_test_signed_extension(&SignedExtensionTestCase {
            validity: Validity {
                from: test_case.validity.from + Duration::from_secs(123),
                ..test_case.validity
            },
            ..test_case.clone()
        })?;
        let certificate = make_test_cert_with_signed_extension(&test_case, extension)?;
        let error = validate_test_signed_extension(&test_case, certificate).unwrap_err();
        defer_on_unwind!(eprintln!("{error}"));
        assert!(error.to_string().starts_with(
            "[VerifySignature] [CertificatePropertiesMismatch] The signed extension content hash doesn't match: not_before was "));
        Ok(())
    }

    #[test]
    fn signed_extension_wrong_validity_to() -> Result<(), Box<dyn Error>> {
        let test_case = SignedExtensionTestCase::new()?;
        let extension = make_test_signed_extension(&SignedExtensionTestCase {
            validity: Validity {
                to: test_case.validity.to + Duration::from_secs(123),
                ..test_case.validity
            },
            ..test_case.clone()
        })?;
        let certificate = make_test_cert_with_signed_extension(&test_case, extension)?;
        let error = validate_test_signed_extension(&test_case, certificate).unwrap_err();
        defer_on_unwind!(eprintln!("{error}"));
        assert!(error.to_string().starts_with(
            "[VerifySignature] [CertificatePropertiesMismatch] The signed extension content hash doesn't match: not_after was "));
        Ok(())
    }

    #[test]
    fn signed_extension_wrong_public_key() -> Result<(), Box<dyn Error>> {
        let test_case = SignedExtensionTestCase::new()?;
        let extension = make_test_signed_extension(&SignedExtensionTestCase {
            public_key: make_key()?,
            ..test_case.clone()
        })?;
        let certificate = make_test_cert_with_signed_extension(&test_case, extension)?;
        let error = validate_test_signed_extension(&test_case, certificate).unwrap_err();
        defer_on_unwind!(eprintln!("{error}"));
        assert!(error.to_string().starts_with(
            "[VerifySignature] [CertificatePropertiesMismatch] The signed extension content hash doesn't match: public_key was '"));
        Ok(())
    }

    #[test]
    fn signed_extension_wrong_signer() -> Result<(), Box<dyn Error>> {
        let test_case = SignedExtensionTestCase::new()?;
        for (t, common_name) in [(true, "Terrazzo Client"), (false, "NOT Terrazzo")] {
            let (signer_certificate, signer_key) = make_named_test_cert(
                &test_case.root,
                &test_case.root_key,
                CertitficateName {
                    country: Some(['D', 'E']),
                    state_or_province: Some("Bayern"),
                    locality: Some("Munich"),
                    organization: Some("Terrazzo"),
                    common_name: Some(common_name),
                },
            )?;
            let extension = make_test_signed_extension(&SignedExtensionTestCase {
                signer_certificate: signer_certificate,
                signer_key: signer_key,
                ..test_case.clone()
            })?;
            let certificate = make_test_cert_with_signed_extension(&test_case, extension)?;
            let result = validate_test_signed_extension(&test_case, certificate);
            if t {
                let () = result?;
            } else {
                let error = result.unwrap_err();
                defer_on_unwind!(eprintln!("{error}"));
                assert_eq!(
                    error.to_string(),
                    "[VerifySigner] [SignerCertificateNameMismatch] The signer certificate name was: NOT Terrazzo");
            }
        }
        Ok(())
    }

    #[test]
    fn signed_extension_untrusted_signer() -> Result<(), Box<dyn Error>> {
        let test_case = SignedExtensionTestCase::new()?;
        let (wrong_signer_certificate, wrong_signer_key) = {
            let (ca, ca_key) = make_test_ca()?;
            make_test_cert(&ca, &ca_key)?
        };
        let extension = make_test_signed_extension(&SignedExtensionTestCase {
            signer_certificate: wrong_signer_certificate,
            signer_key: wrong_signer_key,
            ..test_case.clone()
        })?;
        let certificate = make_test_cert_with_signed_extension(&test_case, extension)?;
        let error = validate_test_signed_extension(&test_case, certificate).unwrap_err();
        defer_on_unwind!(eprintln!("{error}"));
        assert!(error
            .to_string()
            .starts_with("[VerifySignature] [SignedCmsInvalid] "));
        Ok(())
    }

    #[test]
    fn signed_extension_not_a_cert() -> Result<(), Box<dyn Error>> {
        let error =
            validate_signed_extension(&CertificateDer::from_slice("abcd".as_bytes()), None, "")
                .unwrap_err();
        defer_on_unwind!(eprintln!("{error}"));
        assert!(error
            .to_string()
            .starts_with("[X509Certificate] Failed to parse X509Certificate: "));
        Ok(())
    }

    #[test]
    fn signed_extension_missing() -> Result<(), Box<dyn Error>> {
        let test_case = SignedExtensionTestCase::new()?;
        let (certificate, _) = make_test_cert(&test_case.root, &test_case.root_key)?;
        let error = validate_test_signed_extension(&test_case, certificate).unwrap_err();
        defer_on_unwind!(eprintln!("{error}"));
        assert!(error.to_string().starts_with("[SignedExtensionNotFound]"));
        Ok(())
    }

    #[test]
    fn signed_extension_invalid_intermediate() -> Result<(), Box<dyn Error>> {
        let test_case = SignedExtensionTestCase::new()?;
        let (intermediate, _) = make_test_intermediate(&test_case.root, &test_case.root_key)?;
        let extension = make_test_signed_extension(&SignedExtensionTestCase {
            intermediate,
            ..test_case.clone()
        })?;
        let certificate = make_test_cert_with_signed_extension(&test_case, extension)?;
        let error = validate_test_signed_extension(&test_case, certificate).unwrap_err();
        defer_on_unwind!(eprintln!("{error}"));
        assert!(error
            .to_string()
            .starts_with("[VerifySignature] [SignedCmsInvalid] Failed to verify the Signed CMS"));
        Ok(())
    }

    #[derive(Clone)]
    struct SignedExtensionTestCase {
        root: X509,
        root_key: PKey<Private>,
        intermediate: X509,
        intermediate_key: PKey<Private>,
        signer_certificate: X509,
        signer_key: PKey<Private>,
        common_name: String,
        validity: Validity,
        public_key: PKey<Private>,
    }

    impl SignedExtensionTestCase {
        fn new() -> Result<Self, Box<dyn Error>> {
            let (root, root_key) = make_test_ca()?;
            let (intermediate, intermediate_key) = make_test_intermediate(&root, &root_key)?;
            let (signer_certificate, signer_key) =
                make_test_cert(&intermediate, &intermediate_key)?;
            let validity = Validity {
                from: SystemTime::now(),
                to: SystemTime::now() + Duration::from_secs(1) * 3600,
            };
            let public_key = make_key()?;
            Ok(Self {
                root,
                root_key,
                intermediate,
                intermediate_key,
                signer_certificate,
                signer_key,
                common_name: "With signed extension".to_owned(),
                validity,
                public_key,
            })
        }
    }

    fn validate_test_signed_extension(
        test_case: &SignedExtensionTestCase,
        certificate: X509,
    ) -> Result<(), Box<dyn Error>> {
        let store = {
            let mut builder = X509StoreBuilder::new()?;
            builder.add_cert(test_case.root.to_owned())?;
            builder.build()
        };
        let () = validate_signed_extension(
            &CertificateDer::from_slice(&certificate.to_der()?),
            Some(&store),
            "Terrazzo Client",
        )?;
        Ok(())
    }

    fn make_test_cert_with_signed_extension(
        test_case: &SignedExtensionTestCase,
        extension: X509Extension,
    ) -> Result<X509, Box<dyn Error>> {
        let certificate = super::make_cert(
            &test_case.intermediate,
            &test_case.intermediate_key,
            CertitficateName {
                country: Some(['D', 'E']),
                state_or_province: Some("Bayern"),
                locality: Some("Munich"),
                organization: Some("Terrazzo"),
                common_name: Some(&test_case.common_name),
            },
            test_case.validity,
            &test_case.public_key.public_key_to_pem()?.pem_string()?,
            vec![extension],
        )?;
        Ok(certificate)
    }

    fn make_test_signed_extension(
        test_case: &SignedExtensionTestCase,
    ) -> Result<X509Extension, Box<dyn Error>> {
        let mut intermediates = Stack::new()?;
        intermediates.push(test_case.intermediate.clone())?;
        let extension = make_signed_extension(
            &test_case.common_name,
            test_case.validity,
            &test_case.public_key.public_key_to_der()?,
            Some(&intermediates),
            &test_case.signer_certificate,
            &test_case.signer_key,
        )?;
        Ok(extension)
    }

    fn make_test_cert(
        ca: &X509,
        ca_key: &PKey<Private>,
    ) -> Result<(X509, PKey<Private>), Box<dyn Error>> {
        make_named_test_cert(
            ca,
            ca_key,
            CertitficateName {
                country: Some(['D', 'E']),
                state_or_province: Some("Bayern"),
                locality: Some("Munich"),
                organization: Some("Terrazzo"),
                common_name: Some("Terrazzo Client"),
            },
        )
    }

    fn make_named_test_cert(
        ca: &X509,
        ca_key: &PKey<Private>,
        name: CertitficateName,
    ) -> Result<(X509, PKey<Private>), Box<dyn Error>> {
        let private_key = make_key()?;
        let public_key = private_key.public_key_to_pem().pem_string()?;
        let certificate = super::make_cert(
            ca,
            ca_key,
            name,
            Validity {
                from: SystemTime::now(),
                to: SystemTime::now() + Duration::from_secs(1) * 3600,
            },
            &public_key,
            vec![],
        )?;
        Ok((certificate, private_key))
    }

    fn make_test_ca() -> Result<(X509, PKey<Private>), MakeCaError> {
        make_ca(
            CertitficateName {
                country: Some(['D', 'E']),
                state_or_province: Some("Bayern"),
                locality: Some("Munich"),
                organization: Some("Terrazzo"),
                common_name: Some("Terrazzo Test Root CA"),
            },
            Validity {
                from: SystemTime::now(),
                to: SystemTime::now() + Duration::from_secs(1) * 3600,
            },
        )
    }

    fn make_test_intermediate(
        root: &X509Ref,
        root_key: &PKeyRef<Private>,
    ) -> Result<(X509, PKey<Private>), MakeCaError> {
        make_intermediate(
            root,
            root_key,
            CertitficateName {
                common_name: Some("Terrazzo Test intermediate"),
                ..CertitficateName::default()
            },
            Validity {
                from: SystemTime::now(),
                to: SystemTime::now() + Duration::from_secs(1) * 3600,
            },
        )
    }
}
