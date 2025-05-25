use std::sync::Arc;
use std::sync::Mutex;

use nameth::NamedType as _;
use nameth::nameth;
use openssl::pkey::PKey;
use openssl::x509::X509;
use scopeguard::defer;
use tracing::Instrument as _;
use tracing::debug;
use tracing::info;
use tracing::info_span;
use trz_gateway_common::certificate_info::CertificateInfo;
use trz_gateway_common::certificate_info::X509CertificateInfo;
use trz_gateway_common::dynamic_config::has_diff::DiffArc;
use trz_gateway_common::dynamic_config::has_diff::DiffOption;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::security_configuration::common::parse_pem_certificates;

use crate::server::acme::get_certificate::GetAcmeCertificateResult;

use super::AcmeConfig;
use super::AcmeError;
use super::DynamicAcmeConfig;
use super::active_challenges::ActiveChallenges;

#[nameth]
#[derive(Clone)]
pub struct AcmeCertificateConfig {
    acme_config_dyn: DynamicAcmeConfig,
    acme_config: DiffArc<AcmeConfig>,
    state: Arc<std::sync::Mutex<AcmeCertificateState>>,
    active_challenges: ActiveChallenges,
}

impl AcmeCertificateConfig {
    pub fn new(
        acme_config_dyn: DynamicAcmeConfig,
        acme_config: DiffArc<AcmeConfig>,
        active_challenges: ActiveChallenges,
    ) -> Self {
        let state = if let Some(pem) = &acme_config.certificate {
            Arc::new(Mutex::new(
                parse_acme_certificate(&pem)
                    .map(AcmeCertificateState::Done)
                    .unwrap_or_else(|error| AcmeCertificateState::Failed(error.into())),
            ))
        } else {
            Default::default()
        };
        Self {
            acme_config,
            acme_config_dyn,
            state,
            active_challenges,
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

#[derive(Clone, Default)]
enum AcmeCertificateState {
    Done(AcmeCertificate),
    Pending,
    Failed(Arc<AcmeError>),

    #[default]
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
        let error = match state {
            AcmeCertificateState::Done(state) => return Ok(f(state).to_owned()),
            AcmeCertificateState::Pending => return Err(AcmeError::Pending),
            AcmeCertificateState::Failed(acme_error) => AcmeError::Arc(acme_error.clone()),
            AcmeCertificateState::NotSet => AcmeError::Pending,
        };

        *state = AcmeCertificateState::Pending;
        tokio::spawn(self.clone().initialize());
        return Err(error);
    }

    async fn initialize(self) -> Result<(), AcmeError> {
        let acme_certificate: Result<AcmeCertificate, AcmeError> = async move {
            info!("Start");
            defer!(info!("Done"));
            let result = match &self.acme_config.certificate {
                Some(certificate) => GetAcmeCertificateResult {
                    certificate: certificate.clone(),
                    credentials: None,
                },
                None => {
                    self.acme_config
                        .get_certificate(&self.active_challenges)
                        .await?
                }
            };

            if let Some(new_credentials) = result.credentials {
                self.acme_config_dyn.set(|old| {
                    let Some(old) = &**old else {
                        return DiffOption::default();
                    };
                    info!("Update Let's Encrypt account");
                    DiffOption::from(DiffArc::from(AcmeConfig {
                        environment: old.environment.clone(),
                        credentials: Some(new_credentials).into(),
                        contact: old.contact.clone(),
                        domain: old.domain.clone(),
                        certificate: Some(result.certificate.clone()),
                    }))
                });
            } else if Some(&result.certificate) != self.acme_config.certificate.as_ref() {
                self.acme_config_dyn.set(|old| {
                    let Some(old) = &**old else {
                        return DiffOption::default();
                    };
                    info!("Update Let's Encrypt certificate");
                    DiffOption::from(DiffArc::from(AcmeConfig {
                        environment: old.environment.clone(),
                        credentials: old.credentials.clone(),
                        contact: old.contact.clone(),
                        domain: old.domain.clone(),
                        certificate: Some(result.certificate.clone()),
                    }))
                });
            }

            parse_acme_certificate(&result.certificate)
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
