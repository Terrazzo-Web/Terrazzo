use std::io;
use std::io::Write as _;
use std::path::Path;

use openssl::pkcs12::Pkcs12;

/// Prints the PFX's certificate, including extension details, to stdout.
/// Pass an empty password for an unprotected PFX. Private keys are never printed.
#[expect(dead_code, reason = "Standalone helper for inspecting PFX files")]
pub fn print_pfx(path: impl AsRef<Path>, password: &str) -> io::Result<()> {
    let text = load_pfx(path, password)?;
    io::stdout().lock().write_all(text.as_bytes())
}

/// Loads the formatted certificate without exposing the PFX private key.
pub fn load_pfx(path: impl AsRef<Path>, password: &str) -> io::Result<String> {
    let data = std::fs::read(path)?;
    let pfx = Pkcs12::from_der(&data)?.parse2(password)?;
    let certificate = pfx
        .cert
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "PFX contains no certificate"))?;
    crate::converter::certificate_text(&certificate)
}

pub(super) fn is_pfx(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pfx"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use openssl::asn1::Asn1Time;
    use openssl::bn::BigNum;
    use openssl::hash::MessageDigest;
    use openssl::pkey::PKey;
    use openssl::rsa::Rsa;
    use openssl::x509::X509;
    use openssl::x509::X509NameBuilder;

    use super::*;
    use crate::text_editor::file_path::FilePath;
    use crate::text_editor::fsio::File;
    use crate::text_editor::fsio::service;

    #[tokio::test]
    async fn opens_certificate_read_only_and_preserves_pfx_on_save() {
        let key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
        let mut name = X509NameBuilder::new().unwrap();
        name.append_entry_by_text("CN", "PFX viewer test").unwrap();
        let name = name.build();
        let mut certificate = X509::builder().unwrap();
        certificate.set_version(2).unwrap();
        certificate
            .set_serial_number(&BigNum::from_u32(1).unwrap().to_asn1_integer().unwrap())
            .unwrap();
        certificate.set_subject_name(&name).unwrap();
        certificate.set_issuer_name(&name).unwrap();
        certificate.set_pubkey(&key).unwrap();
        certificate
            .set_not_before(&Asn1Time::days_from_now(0).unwrap())
            .unwrap();
        certificate
            .set_not_after(&Asn1Time::days_from_now(1).unwrap())
            .unwrap();
        certificate.sign(&key, MessageDigest::sha256()).unwrap();
        let certificate = certificate.build();
        let data = Pkcs12::builder()
            .pkey(&key)
            .cert(&certificate)
            .build2("")
            .unwrap()
            .to_der()
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = FilePath {
            base: Arc::<Path>::from(directory.path()),
            file: Arc::<Path>::from(Path::new("certificate.PFX")),
        };
        std::fs::write(path.full_path(), &data).unwrap();
        let file = service::load_file(path.clone()).await.unwrap().unwrap();
        let File::TextFile {
            content,
            read_only,
            original,
            ..
        } = file
        else {
            panic!("expected a text certificate view");
        };
        assert!(read_only);
        assert!(original.is_none());
        // PKCS#12 parsing associates the key with the certificate, which can
        // change OpenSSL's public-key formatting without changing the DER.
        let decoded = Pkcs12::from_der(&data)
            .unwrap()
            .parse2("")
            .unwrap()
            .cert
            .unwrap();
        assert_eq!(decoded.to_der().unwrap(), certificate.to_der().unwrap());
        assert_eq!(
            &*content,
            crate::converter::certificate_text(&decoded).unwrap()
        );
        assert!(!content.contains("PRIVATE KEY"));
        assert!(matches!(
            service::load_file_metadata(path.clone()).await.unwrap(),
            Some(File::TextFile {
                read_only: true,
                ..
            })
        ));
        assert!(
            service::store_file(path.clone(), "replacement".into())
                .await
                .is_err()
        );
        assert_eq!(std::fs::read(path.full_path()).unwrap(), data);
    }

    #[test]
    fn rejects_invalid_pfx() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"not a PFX").unwrap();
        assert!(load_pfx(file.path(), "").is_err());
    }
}
