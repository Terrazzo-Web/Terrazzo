//! Self-signed Root CA.
//!
//! This Root CA is not used as the trust anchor, it is used to issue client
//! certificates but the security comes from the signed extension.

use std::io::ErrorKind;
use std::path::Path;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use trz_gateway_common::certificate_info::CertificateError;
use trz_gateway_common::certificate_info::CertificateInfo;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificate;
use trz_gateway_common::x509::PemAsStringError;
use trz_gateway_common::x509::PemString as _;
use trz_gateway_common::x509::ca::MakeCaError;
use trz_gateway_common::x509::ca::make_ca;
use trz_gateway_common::x509::name::CertitficateName;
use trz_gateway_common::x509::validity::Validity;

/// Default implementation to load or create a Root CA stored as PEM file on disk.
pub fn load_root_ca(
    name: CertitficateName,
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
                .map_err(|error| RootCaConfigError::Load(error, format!("{root_ca_path:?}")))?;
            Ok(root_ca.into())
        }
        CertificateInfo {
            certificate: false,
            private_key: false,
        } => {
            let root_ca = make_ca(name, default_validity).map_err(Box::new)?;
            let root_ca_pem = CertificateInfo {
                certificate: root_ca.certificate.to_pem(),
                private_key: root_ca.private_key.private_key_to_pem_pkcs8(),
            }
            .try_map(|maybe_pem| maybe_pem.pem_string())?;
            let _: CertificateInfo<()> = root_ca_path
                .zip(root_ca_pem.as_ref())
                .try_map(write_pem_file)
                .map_err(|error| RootCaConfigError::Store(error, format!("{root_ca_path:?}")))?;

            #[cfg(unix)]
            {
                use std::fs::Permissions;
                use std::os::unix::fs::PermissionsExt as _;
                let permissions = Permissions::from_mode(0o600);
                std::fs::set_permissions(root_ca_path.private_key, permissions)
                    .map_err(RootCaConfigError::SetPrivateKeyFilePermissions)?;
            }

            Ok(root_ca_pem.into())
        }
        CertificateInfo {
            certificate: root_ca_exists,
            private_key: private_key_exists,
        } => {
            return Err(RootCaConfigError::InconsistentState {
                root_ca_exists,
                private_key_exists,
            });
        }
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum RootCaConfigError {
    #[error("[{n}] Failed to load certificate from '{1}': {0}", n = self.name())]
    Load(CertificateError<std::io::Error>, String),

    #[error("[{n}] Failed to store certificate into '{1}': {0}", n = self.name())]
    Store(CertificateError<std::io::Error>, String),

    #[error("[{n}] Inconsistent state: root_ca_exists:{root_ca_exists} private_key_exists:{private_key_exists}", n = self.name())]
    InconsistentState {
        root_ca_exists: bool,
        private_key_exists: bool,
    },

    #[error("[{n}] {0}", n = self.name())]
    MakeCa(#[from] Box<MakeCaError>),

    #[error("[{n}] {0}", n = self.name())]
    PemString(#[from] CertificateError<PemAsStringError>),

    #[cfg(unix)]
    #[error("[{n}] {0}", n = self.name())]
    SetPrivateKeyFilePermissions(std::io::Error),
}

fn write_pem_file((path, pem): (&Path, &str)) -> Result<(), std::io::Error> {
    let parent_dir = path.parent().ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("Failed to get parent folder of: {path:?}"),
        )
    })?;
    std::fs::create_dir_all(parent_dir)?;
    std::fs::write(path, pem)
}
