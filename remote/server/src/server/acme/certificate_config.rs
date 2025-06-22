//! [CertificateConfig] based on Let's encrypt certificates.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;

use nameth::NamedType as _;
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
pub struct AcmeCertificateConfig {
    acme_config_dyn: DynamicAcmeConfig,
    acme_config: DiffArc<AcmeConfig>,
    state: Arc<std::sync::Mutex<AcmeCertificateState>>,
    active_challenges: ActiveChallenges,
    certificate_renewal_threshold: Duration,
}

impl AcmeCertificateConfig {
    pub fn new(
        acme_config_dyn: DynamicAcmeConfig,
        acme_config: DiffArc<AcmeConfig>,
        active_challenges: ActiveChallenges,
        certificate_renewal_threshold: Duration,
    ) -> Self {
        let state = if let Some(pem) = &acme_config.certificate {
            Arc::new(Mutex::new(
                parse_acme_certificate(&pem)
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

impl CertificateConfig for AcmeCertificateConfig {
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

impl AcmeCertificateConfig {
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
            let result = if let (CertificateInitStrategy::GetOrInit, Some(certificate)) =
                (strategy, &self.acme_config.certificate)
            {
                debug!("Using a cached certificate from configuration");
                GetAcmeCertificateResult {
                    certificate: certificate.clone(),
                    credentials: None,
                }
            } else {
                debug!("Obtain a brand new certificate");
                self.acme_config
                    .get_certificate(&self.active_challenges)
                    .await?
            };

            let acme_certificate =
                parse_acme_certificate(&result.certificate).inspect_err(|error| {
                    self.acme_config_dyn.set(|old| {
                        warn!("The cached certificate was invalid: {error}");
                        let Some(old) = &**old else {
                            return DiffOption::default();
                        };
                        DiffOption::from(DiffArc::from(AcmeConfig {
                            certificate: None,
                            ..AcmeConfig::clone(&old)
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
                        certificate: Some(result.certificate.clone()),
                        ..AcmeConfig::clone(&old)
                    }))
                });
            } else if Some(&result.certificate) != self.acme_config.certificate.as_ref() {
                self.acme_config_dyn.set(|old| {
                    let Some(old) = &**old else {
                        return DiffOption::default();
                    };
                    info!("Update Let's Encrypt certificate");
                    DiffOption::from(DiffArc::from(AcmeConfig {
                        certificate: Some(result.certificate.clone()),
                        ..AcmeConfig::clone(&old)
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

fn parse_acme_certificate(pem: &CertificateInfo<String>) -> Result<AcmeCertificate, AcmeError> {
    let mut chain = parse_pem_certificates(&pem.certificate);
    let certificate = chain.next().ok_or(AcmeError::CertificateChain)??;
    let mut intermediates = vec![];
    for intermediate in chain {
        intermediates.push(intermediate?);
    }
    Ok(AcmeCertificate {
        intermediates: Arc::new(intermediates),
        certificate: Arc::new(CertificateInfo {
            certificate,
            private_key: PKey::private_key_from_pem(pem.private_key.as_bytes())?,
        }),
    })
}

impl std::fmt::Debug for AcmeCertificateConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(AcmeCertificateConfig::type_name())
            .field("environment", &self.acme_config.environment)
            .finish()
    }
}

enum CertificateInitStrategy {
    Force,
    GetOrInit,
}
