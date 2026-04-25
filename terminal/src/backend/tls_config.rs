use std::time::Duration;
use std::time::SystemTime;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use trz_gateway_common::security_configuration::SecurityConfig;
use trz_gateway_common::security_configuration::certificate::CertificateConfig as _;
use trz_gateway_common::security_configuration::certificate::cache::CachedCertificate;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificate;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificateError;
use trz_gateway_common::unwrap_infallible::UnwrapInfallible as _;
use trz_gateway_common::x509::PemAsStringError;
use trz_gateway_common::x509::PemString as _;
use trz_gateway_common::x509::cert::MakeCertError;
use trz_gateway_common::x509::cert::make_cert;
use trz_gateway_common::x509::key::MakeKeyError;
use trz_gateway_common::x509::key::make_key;
use trz_gateway_common::x509::name::CertitficateName;
use trz_gateway_common::x509::time::Asn1ToSystemTimeError;
use trz_gateway_common::x509::validity::Validity;
use trz_gateway_common::x509::validity::ValidityError;

use super::root_ca_config::PrivateRootCa;

pub fn make_tls_config(
    root_ca: &PrivateRootCa,
) -> Result<SecurityConfig<PrivateRootCa, CachedCertificate>, TlsConfigError> {
    let root_ca_x509 = root_ca.certificate().unwrap_infallible();
    let validity = root_ca_x509.certificate.as_ref().try_into()?;

    let certificate_key = make_key()?;
    let certificate = make_cert(
        (*root_ca_x509).as_ref(),
        CertitficateName {
            organization: Some("Terrazzo"),
            common_name: Some("localhost"),
            ..CertitficateName::default()
        },
        validity,
        &certificate_key
            .public_key_to_pem()
            .pem_string()
            .map_err(TlsConfigError::PublicKeyPem)?,
        vec![],
    )?;

    let certificate_pem = certificate.to_pem().pem_string();
    let private_key_pem = certificate_key.private_key_to_pem_pkcs8().pem_string();
    Ok(SecurityConfig {
        trusted_store: root_ca.clone(),
        certificate: PemCertificate {
            intermediates_pem: String::new(),
            certificate_pem: certificate_pem.map_err(TlsConfigError::CertificatePem)?,
            private_key_pem: private_key_pem.map_err(TlsConfigError::PrivateKeyPem)?,
        }
        .cache()?,
    })
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum TlsConfigError {
    #[error("[{n}] {0}", n = self.name())]
    ValidityError(#[from] ValidityError<Asn1ToSystemTimeError>),

    #[error("[{n}] {0}", n = self.name())]
    MakeKey(#[from] MakeKeyError),

    #[error("[{n}] {0}", n = self.name())]
    MakeCertificate(#[from] MakeCertError),

    #[error("[{n}] {0}", n = self.name())]
    CertificatePem(PemAsStringError),

    #[error("[{n}] {0}", n = self.name())]
    PublicKeyPem(PemAsStringError),

    #[error("[{n}] {0}", n = self.name())]
    PrivateKeyPem(PemAsStringError),

    #[error("[{n}] {0}", n = self.name())]
    PemCertificateError(#[from] PemCertificateError),
}
