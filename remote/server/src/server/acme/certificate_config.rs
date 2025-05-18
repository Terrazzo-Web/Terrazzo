use std::sync::Arc;

use openssl::pkey::PKey;
use openssl::x509::X509;
use trz_gateway_common::certificate_info::CertificateInfo;
use trz_gateway_common::certificate_info::X509CertificateInfo;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::security_configuration::common::parse_pem_certificates;

use super::AcmeConfig;
use super::AcmeError;

pub struct AcmeCertificateConfig {
    state: Arc<std::sync::Mutex<(AcmeConfig, AcmeCertificateState)>>,
}

impl CertificateConfig for AcmeCertificateConfig {
    type Error = AcmeError;

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
        let (acme_config, maybe_state) = &mut *lock;
        let error = match maybe_state {
            AcmeCertificateState::Done(state) => return Ok(f(state).to_owned()),
            AcmeCertificateState::Pending => return Err(AcmeError::Pending),
            AcmeCertificateState::Failed(acme_error) => AcmeError::Arc(acme_error.clone()),
            AcmeCertificateState::NotSet => AcmeError::Pending,
        };
        *maybe_state = AcmeCertificateState::Pending;
        tokio::spawn(Self::initialize(acme_config.clone(), self.state.clone()));
        return Err(error);
    }

    async fn initialize(
        acme_config: AcmeConfig,
        state: Arc<std::sync::Mutex<(AcmeConfig, AcmeCertificateState)>>,
    ) -> Result<(), AcmeError> {
        let acme_certificate: Result<AcmeCertificate, AcmeError> = async move {
            let result = acme_config.get_certificate().await?;
            let mut chain = parse_pem_certificates(&result.certificate);
            let certificate = chain.next().ok_or(AcmeError::CertificateChain)??;
            let mut intermediates = vec![];
            for intermediate in chain {
                intermediates.push(intermediate?);
            }
            Ok(AcmeCertificate {
                intermediates: Arc::new(intermediates),
                certificate: Arc::new(CertificateInfo {
                    certificate,
                    private_key: PKey::private_key_from_pem(result.private_key.as_bytes())?,
                }),
            })
        }
        .await;
        state.lock().unwrap().1 = match acme_certificate {
            Ok(acme_certificate) => AcmeCertificateState::Done(acme_certificate),
            Err(error) => AcmeCertificateState::Failed(Arc::new(error)),
        };
        Ok(())
    }
}
