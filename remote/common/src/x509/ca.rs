use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use openssl::x509::X509Builder;
use openssl::x509::extension::BasicConstraints;
use openssl::x509::extension::KeyUsage;

use super::common_fields::SetCommonFieldsError;
use super::common_fields::set_akid;
use super::common_fields::set_common_fields;
use super::key::MakeKeyError;
use super::key::make_key;
use super::name::CertitficateName;
use super::name::MakeNameError;
use super::name::make_name;
use super::validity::Validity;
use crate::certificate_info::CertificateInfo;
use crate::certificate_info::X509CertificateInfo;
use crate::certificate_info::X509CertificateInfoRef;

/// Makes a Root CA certificate.
pub fn make_ca(
    subject_name: CertitficateName,
    validity: Validity,
) -> Result<X509CertificateInfo, MakeCaError> {
    make_impl(None, subject_name, validity)
}

/// Makes an intermediate CA certificate.
pub fn make_intermediate(
    root: X509CertificateInfoRef,
    subject_name: CertitficateName,
    validity: Validity,
) -> Result<X509CertificateInfo, MakeCaError> {
    make_impl(Some(root), subject_name, validity)
}

fn make_impl(
    issuer: Option<X509CertificateInfoRef>,
    subject_name: CertitficateName,
    validity: Validity,
) -> Result<X509CertificateInfo, MakeCaError> {
    let key = make_key()?;

    let mut builder = X509Builder::new().map_err(MakeCaError::NewBuilder)?;

    builder
        .set_pubkey(&key)
        .map_err(MakeCaError::SetPublicKey)?;
    let subject_name = make_name(subject_name)?;
    let (issuer_name, issuer_key) = issuer
        .map(|issuer| (issuer.certificate.subject_name(), issuer.private_key))
        .unwrap_or((&subject_name, &key));
    set_common_fields(&mut builder, issuer_name, &subject_name, validity)?;

    (|| {
        let basic_constraints = BasicConstraints::new().critical().ca().build()?;
        builder.append_extension(basic_constraints)?;
        Ok(())
    })()
    .map_err(MakeCaError::BasicConstraints)?;

    (|| {
        let key_usage = KeyUsage::new()
            .critical()
            .key_cert_sign()
            .crl_sign()
            .build()?;
        builder.append_extension(key_usage)?;
        Ok(())
    })()
    .map_err(MakeCaError::KeyUsage)?;

    if let Some(issuer) = issuer {
        set_akid(issuer.certificate, &mut builder).map_err(MakeCaError::AuthorityKeyIdentifier)?;
    }

    builder
        .sign(issuer_key, openssl::hash::MessageDigest::sha256())
        .map_err(MakeCaError::Sign)?;

    let root_cert = builder.build();

    Ok(CertificateInfo {
        certificate: root_cert,
        private_key: key,
    })
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MakeCaError {
    #[error("[{n}] {0}", n = self.name())]
    MakeKey(#[from] MakeKeyError),

    #[error("[{n}] Failed to create a new X509 Certificate builder: {0}", n = self.name())]
    NewBuilder(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    MakeName(#[from] MakeNameError),

    #[error("[{n}] Failed to set the public key: {0}", n = self.name())]
    SetPublicKey(ErrorStack),

    #[error("[{n}] Failed to set AKID: {0}", n = self.name())]
    AuthorityKeyIdentifier(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    SetCommonFieldsError(#[from] SetCommonFieldsError),

    #[error("[{n}] Failed to set basic constraints: {0}", n = self.name())]
    BasicConstraints(ErrorStack),

    #[error("[{n}] Failed to set key usage: {0}", n = self.name())]
    KeyUsage(ErrorStack),

    #[error("[{n}] Failed to sign the certificate: {0}", n = self.name())]
    Sign(ErrorStack),
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::time::Duration;
    use std::time::SystemTime;

    use openssl::sign::Signer;
    use openssl::sign::Verifier;

    use super::super::name::CertitficateName;
    use super::MakeCaError;
    use crate::x509::PemString as _;
    use crate::x509::validity::Validity;

    #[test]
    fn make_ca() -> Result<(), Box<dyn Error>> {
        Ok({
            let ca = super::make_ca(
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
            )?;
            let text = ca.certificate.to_text().pem_string().unwrap();
            let _debug = scopeguard::guard_on_unwind((), |_| {
                println!("Certificate is\n{text}");
            });

            assert!(text.contains("Signature Algorithm: ecdsa-with-SHA256"));
            assert!(text.contains(
                "Issuer: C=DE, ST=Bayern, L=Munich, O=Terrazzo, CN=Terrazzo Test Root CA"
            ));
            assert!(text.contains(
                "Issuer: C=DE, ST=Bayern, L=Munich, O=Terrazzo, CN=Terrazzo Test Root CA"
            ));
            assert!(text.contains("CA:TRUE"));
            assert!(text.contains("X509v3 Subject Key Identifier"));
            assert!(
                !text
                    .to_ascii_uppercase()
                    .contains("DA:39:A3:EE:5E:6B:4B:0D:32:55:BF:EF:95:60:18:90:AF:D8:07:09")
            );
        })
    }

    #[test]
    fn sign_payload() -> Result<(), Box<dyn Error>> {
        const DATA: &str = "Hello, world! 😃";

        Ok({
            let ca = super::make_ca(
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
            )?;
            let public_key = ca.certificate.public_key()?;

            let signature = {
                let mut signer = Signer::new_without_digest(&ca.private_key)?;
                signer.update(DATA.as_bytes())?;
                signer.sign_to_vec()?
            };

            {
                let mut verifier = Verifier::new_without_digest(&public_key)?;
                verifier.update(DATA.as_bytes())?;
                assert!(verifier.verify(&signature)?);
            }

            let ca = super::make_ca(
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
            )?;
            let public_key = ca.certificate.public_key()?;
            let mut verifier = Verifier::new_without_digest(&public_key)?;
            verifier.update(DATA.as_bytes())?;
            assert_eq!(false, verifier.verify(&signature)?);
        })
    }

    #[test]
    fn invalid_name() -> Result<(), Box<dyn Error>> {
        Ok({
            let too_long: String = (0..200).map(|_| 'X').collect();
            let Err(MakeCaError::MakeName(..)) = super::make_ca(
                CertitficateName {
                    country: Some(['D', 'E']),
                    state_or_province: Some("Bayern"),
                    locality: Some("Munich"),
                    organization: Some("Terrazzo"),
                    common_name: Some(&too_long),
                },
                Validity {
                    from: SystemTime::now(),
                    to: SystemTime::now() + Duration::from_secs(1) * 3600,
                },
            ) else {
                panic!()
            };
        })
    }
}
