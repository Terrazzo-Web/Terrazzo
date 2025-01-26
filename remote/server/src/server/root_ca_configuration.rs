use std::path::Path;

use nameth::nameth;
use nameth::NamedEnumValues as _;

use crate::security_configuration::SecurityConfig;
use crate::utils::x509::ca::make_ca;
use crate::utils::x509::ca::MakeCaError;
use crate::utils::x509::name::CertitficateName;
use crate::utils::x509::validity::Validity;
use crate::utils::x509::PemAsStringError;
use crate::utils::x509::PemString as _;

pub struct RootCaConfig {
    root_ca: String,
    private_key: String,
}

impl RootCaConfig {
    pub fn load(
        name: String,
        root_ca: impl AsRef<Path>,
        private_key: impl AsRef<Path>,
        default_validity: Validity,
    ) -> Result<Self, RootCaConfigError> {
        let root_ca = root_ca.as_ref();
        let private_key = private_key.as_ref();

        match (root_ca.exists(), private_key.exists()) {
            (true, true) => {
                let root_ca =
                    std::fs::read_to_string(root_ca).map_err(RootCaConfigError::LoadRootCa)?;
                let private_key = std::fs::read_to_string(private_key)
                    .map_err(RootCaConfigError::LoadPrivateKey)?;

                Ok(Self {
                    root_ca,
                    private_key,
                })
            }
            (false, false) => {
                let (certificate, private_key) = make_ca(
                    CertitficateName {
                        common_name: Some(name.as_str()),
                        ..CertitficateName::default()
                    },
                    default_validity,
                )?;
                Ok(Self {
                    root_ca: certificate.to_pem().pem_string()?,
                    private_key: private_key.private_key_to_pem_pkcs8().pem_string()?,
                })
            }
            (root_ca_exists, private_key_exists) => {
                return Err(RootCaConfigError::InconsistentState {
                    root_ca_exists,
                    private_key_exists,
                })
            }
        }
    }
}

impl SecurityConfig for RootCaConfig {
    fn certificate_pem(&self) -> &str {
        &self.root_ca
    }

    fn private_key_pem(&self) -> &str {
        &self.private_key
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum RootCaConfigError {
    #[error("[{n}] Failed to load certificate: {0}", n = self.name())]
    LoadRootCa(std::io::Error),

    #[error("[{n}] Failed to load private key: {0}", n = self.name())]
    LoadPrivateKey(std::io::Error),

    #[error("[{n}] Inconsistent state: root_ca_exists:{root_ca_exists} private_key_exists:{private_key_exists}", n = self.name())]
    InconsistentState {
        root_ca_exists: bool,
        private_key_exists: bool,
    },

    #[error("[{n}] {0}", n = self.name())]
    MakeCa(#[from] MakeCaError),

    #[error("[{n}] {0}", n = self.name())]
    PemString(#[from] PemAsStringError),
}
