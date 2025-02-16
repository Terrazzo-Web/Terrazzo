use std::path::Path;

use nameth::nameth;
use nameth::NamedEnumValues as _;
use trz_gateway_common::certificate_info::CertificateError;
use trz_gateway_common::certificate_info::CertificateInfo;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificate;
use trz_gateway_common::x509::ca::make_ca;
use trz_gateway_common::x509::ca::MakeCaError;
use trz_gateway_common::x509::name::CertitficateName;
use trz_gateway_common::x509::validity::Validity;
use trz_gateway_common::x509::PemAsStringError;
use trz_gateway_common::x509::PemString as _;

pub fn load_root_ca(
    name: String,
    root_ca_path: CertificateInfo<impl AsRef<Path>>,
    default_validity: Validity,
) -> Result<PemCertificate, RootCaConfigError> {
    let root_ca_path = root_ca_path.as_ref();
    match root_ca_path.map(|path| path.exists()) {
        CertificateInfo {
            certificate: true,
            private_key: true,
        } => {
            let root_ca = root_ca_path
                .try_map(std::fs::read_to_string)
                .map_err(RootCaConfigError::Load)?;
            Ok(root_ca.into())
        }
        CertificateInfo {
            certificate: false,
            private_key: false,
        } => {
            let (certificate, private_key) = make_ca(
                CertitficateName {
                    common_name: Some(name.as_str()),
                    ..CertitficateName::default()
                },
                default_validity,
            )
            .map_err(Box::new)?;
            let pem_certificate = PemCertificate {
                certificate_pem: certificate.to_pem().pem_string()?,
                private_key_pem: private_key.private_key_to_pem_pkcs8().pem_string()?,
                intermediates_pem: String::default(),
            };
            let _: CertificateInfo<()> = root_ca_path
                .zip(CertificateInfo {
                    certificate: &pem_certificate.certificate_pem,
                    private_key: &pem_certificate.private_key_pem,
                })
                .try_map(|(path, pem)| std::fs::write(path, pem))
                .map_err(RootCaConfigError::Store)?;
            Ok(pem_certificate)
        }
        CertificateInfo {
            certificate: root_ca_exists,
            private_key: private_key_exists,
        } => {
            return Err(RootCaConfigError::InconsistentState {
                root_ca_exists,
                private_key_exists,
            })
        }
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum RootCaConfigError {
    #[error("[{n}] Failed to load certificate: {0}", n = self.name())]
    Load(CertificateError<std::io::Error>),

    #[error("[{n}] Failed to store certificate: {0}", n = self.name())]
    Store(CertificateError<std::io::Error>),

    #[error("[{n}] Inconsistent state: root_ca_exists:{root_ca_exists} private_key_exists:{private_key_exists}", n = self.name())]
    InconsistentState {
        root_ca_exists: bool,
        private_key_exists: bool,
    },

    #[error("[{n}] {0}", n = self.name())]
    MakeCa(#[from] Box<MakeCaError>),

    #[error("[{n}] {0}", n = self.name())]
    PemString(#[from] PemAsStringError),
}
