//! [CertificateConfig] based on Let's encrypt certificates.

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;

use nameth::nameth;
use openssl::pkey::PKey;
use openssl::x509::X509;
use scopeguard::defer;
use tracing::Instrument as _;
use tracing::debug;
use tracing::info;
use tracing::info_span;
use tracing::warn;
use trz_gateway_common::certificate_info::CertificateInfo;
use trz_gateway_common::certificate_info::X509CertificateInfo;
use trz_gateway_common::dynamic_config::has_diff::DiffArc;
use trz_gateway_common::dynamic_config::has_diff::DiffOption;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::security_configuration::common::parse_pem_certificates;
use trz_gateway_common::x509::time::asn1_to_system_time;

use super::AcmeConfig;
use super::AcmeError;
use super::DynamicAcmeConfig;
use super::active_challenges::ActiveChallenges;
use crate::server::acme::get_certificate::GetAcmeCertificateResult;

/// A [CertificateConfig] generated with Let's Encrypt.
#[nameth]
#[derive(Clone)]
pub struct AcmeCertificateConfig<P = PathBuf> {
    acme_config_dyn: DynamicAcmeConfig<P>,
    acme_config: DiffArc<AcmeConfig<P>>,
    state: Arc<std::sync::Mutex<AcmeCertificateState>>,
    active_challenges: ActiveChallenges,
    certificate_renewal_threshold: Duration,
}

impl<P> AcmeCertificateConfig<P>
where
    P: AsRef<Path> + Clone,
{
    pub fn new(
        acme_config_dyn: DynamicAcmeConfig<P>,
        acme_config: DiffArc<AcmeConfig<P>>,
        active_challenges: ActiveChallenges,
        certificate_renewal_threshold: Duration,
    ) -> Self {
        let state = if let Some(certificate) = &acme_config.certificate {
            Arc::new(Mutex::new(
                load_acme_certificate(certificate, acme_config.private_key_path())
                    .map(AcmeCertificateState::Done)
                    .unwrap_or_else(|error| AcmeCertificateState::Failed(error.into())),
            ))
        } else {
            Arc::new(Mutex::new(AcmeCertificateState::NotSet))
        };
        Self {
            acme_config,
            acme_config_dyn,
            state,
            active_challenges,
            certificate_renewal_threshold,
        }
    }
}

impl<P> CertificateConfig for AcmeCertificateConfig<P>
where
    P: AsRef<Path> + Clone + Send + Sync + 'static,
{
    type Error = AcmeError;

    // TODO get intermediates+certificate should be atomic.

    fn intermediates(&self) -> Result<Arc<Vec<X509>>, Self::Error> {
        return self.get_or_initialize(|state| &state.intermediates);
    }

    fn certificate(&self) -> Result<Arc<X509CertificateInfo>, Self::Error> {
        return self.get_or_initialize(|state| &state.certificate);
    }
}

#[derive(Clone)]
enum AcmeCertificateState {
    Done(AcmeCertificate),
    Renewing(AcmeCertificate),
    Pending,
    Failed(Arc<AcmeError>),
    NotSet,
}

#[derive(Clone)]
struct AcmeCertificate {
    intermediates: Arc<Vec<X509>>,
    certificate: Arc<X509CertificateInfo>,
}

impl<P> AcmeCertificateConfig<P>
where
    P: AsRef<Path> + Clone + Send + Sync + 'static,
{
    fn get_or_initialize<R: Clone>(
        &self,
        f: impl FnOnce(&AcmeCertificate) -> &R,
    ) -> Result<R, AcmeError> {
        let mut lock = self.state.lock().unwrap();
        let state = &mut *lock;
        let (result, new_state, strategy) = match state {
            AcmeCertificateState::Done(done) => {
                let not_after = asn1_to_system_time(done.certificate.certificate.not_after())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                let now = SystemTime::now();
                if not_after <= now + self.certificate_renewal_threshold {
                    debug!(?not_after, ?now, "The certificate is eligible for renewal");
                    let result = Ok(f(done).to_owned());
                    let new_state = AcmeCertificateState::Renewing(done.clone());
                    (result, new_state, CertificateInitStrategy::Force)
                } else {
                    debug!(?not_after, renewal_in = ?not_after.duration_since(now), "The certificate is not eligible for renewal");
                    return Ok(f(done).to_owned());
                }
            }
            AcmeCertificateState::Renewing(old_certificate) => {
                return Ok(f(old_certificate).to_owned());
            }
            AcmeCertificateState::Pending => return Err(AcmeError::Pending),
            AcmeCertificateState::Failed(acme_error) => (
                Err(AcmeError::Arc(acme_error.clone())),
                AcmeCertificateState::Pending,
                CertificateInitStrategy::GetOrInit,
            ),
            AcmeCertificateState::NotSet => (
                Err(AcmeError::Pending),
                AcmeCertificateState::Pending,
                CertificateInitStrategy::GetOrInit,
            ),
        };

        *state = new_state;
        tokio::spawn(self.clone().initialize(strategy).in_current_span());
        return result;
    }

    async fn initialize(self, strategy: CertificateInitStrategy) -> Result<(), AcmeError> {
        let acme_certificate: Result<AcmeCertificate, AcmeError> = async move {
            info!("Start");
            defer!(info!("Done"));
            let cached = if let (CertificateInitStrategy::GetOrInit, Some(certificate)) =
                (strategy, &self.acme_config.certificate)
            {
                match read_private_key(self.acme_config.private_key_path()) {
                    Ok(private_key) => {
                        debug!("Using a cached certificate from configuration");
                        Some(GetAcmeCertificateResult {
                            certificate: CertificateInfo {
                                certificate: certificate.clone(),
                                private_key,
                            },
                            credentials: None,
                        })
                    }
                    Err(error) => {
                        warn!("The cached certificate's private key could not be loaded: {error}");
                        None
                    }
                }
            } else {
                None
            };
            let (result, is_cached) = if let Some(cached) = cached {
                (cached, true)
            } else {
                debug!("Obtain a brand new certificate");
                (
                    self.acme_config
                        .get_certificate(&self.active_challenges)
                        .await?,
                    false,
                )
            };

            if !is_cached {
                write_private_key(
                    self.acme_config.private_key_path(),
                    result.certificate.private_key.as_bytes(),
                )?;
            }

            let acme_certificate = parse_acme_certificate(
                &result.certificate.certificate,
                result.certificate.private_key.as_bytes(),
            )
            .inspect_err(|error| {
                self.acme_config_dyn.set(|old| {
                    warn!("The cached certificate was invalid: {error}");
                    let Some(old) = &**old else {
                        return DiffOption::default();
                    };
                    DiffOption::from(DiffArc::from(AcmeConfig {
                        certificate: None,
                        ..AcmeConfig::clone(old)
                    }))
                });
            })?;

            if let Some(new_credentials) = result.credentials {
                self.acme_config_dyn.set(|old| {
                    let Some(old) = &**old else {
                        return DiffOption::default();
                    };
                    info!("Update Let's Encrypt account");
                    DiffOption::from(DiffArc::from(AcmeConfig {
                        credentials: Some(new_credentials).into(),
                        certificate: Some(result.certificate.certificate.clone()),
                        ..AcmeConfig::clone(old)
                    }))
                });
            } else if Some(&result.certificate.certificate) != self.acme_config.certificate.as_ref()
            {
                self.acme_config_dyn.set(|old| {
                    let Some(old) = &**old else {
                        return DiffOption::default();
                    };
                    info!("Update Let's Encrypt certificate");
                    DiffOption::from(DiffArc::from(AcmeConfig {
                        certificate: Some(result.certificate.certificate.clone()),
                        ..AcmeConfig::clone(old)
                    }))
                });
            }

            return Ok(acme_certificate);
        }
        .instrument(info_span!("Initializing certificate"))
        .await;
        *self.state.lock().unwrap() = match acme_certificate {
            Ok(acme_certificate) => {
                debug!("Got certificate");
                AcmeCertificateState::Done(acme_certificate)
            }
            Err(error) => {
                debug!("Failed to get certificate: {error}");
                AcmeCertificateState::Failed(Arc::new(error))
            }
        };
        Ok(())
    }
}

fn load_acme_certificate(
    certificate: &str,
    private_key_path: &Path,
) -> Result<AcmeCertificate, AcmeError> {
    let private_key = read_private_key(private_key_path)?;
    parse_acme_certificate(certificate, private_key.as_bytes())
}

fn parse_acme_certificate(
    certificate: &str,
    private_key: &[u8],
) -> Result<AcmeCertificate, AcmeError> {
    let mut chain = parse_pem_certificates(certificate);
    let certificate = chain.next().ok_or(AcmeError::CertificateChain)??;
    let mut intermediates = vec![];
    for intermediate in chain {
        intermediates.push(intermediate?);
    }
    Ok(AcmeCertificate {
        intermediates: Arc::new(intermediates),
        certificate: Arc::new(CertificateInfo {
            certificate,
            private_key: PKey::private_key_from_pem(private_key)?,
        }),
    })
}

fn read_private_key(path: &Path) -> Result<String, AcmeError> {
    std::fs::read_to_string(path).map_err(|source| AcmeError::ReadPrivateKey {
        path: path.to_owned(),
        source,
    })
}

fn write_private_key(path: &Path, private_key: &[u8]) -> Result<(), AcmeError> {
    let result = (|| -> std::io::Result<()> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        file.write_all(private_key)?;
        #[cfg(unix)]
        file.set_permissions({
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::Permissions::from_mode(0o600)
        })?;
        Ok(())
    })();
    result.map_err(|source| AcmeError::WritePrivateKey {
        path: path.to_owned(),
        source,
    })
}

impl<P: std::fmt::Debug> std::fmt::Debug for AcmeCertificateConfig<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(ACME_CERTIFICATE_CONFIG)
            .field("environment", &self.acme_config.environment)
            .finish()
    }
}

#[derive(Clone, Copy)]
enum CertificateInitStrategy {
    Force,
    GetOrInit,
}

#[cfg(test)]
mod tests {
    use super::write_private_key;

    #[test]
    fn private_key_is_written_to_a_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("nested/letsencrypt.key");

        write_private_key(&path, b"private key").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"private key");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
