use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;
use std::time::Duration;

use tokio::sync::watch;
use trz_gateway_common::security_configuration::SecurityConfig;
use trz_gateway_common::security_configuration::either::EitherConfig;
use trz_gateway_common::security_configuration::trusted_store::native::NativeTrustedStoreConfig;
use trz_gateway_server::server::acme::AcmeConfig;
use trz_gateway_server::server::acme::DynamicAcmeConfig;
use trz_gateway_server::server::acme::active_challenges::ActiveChallenges;
use trz_gateway_server::server::acme::certificate_config::AcmeCertificateConfig;

use super::DynConfig;
use super::WatchConfig as _;
use crate::backend::root_ca_config::PrivateRootCa;
use crate::backend::server_config::TlsConfig;
use crate::backend::tls_config::make_tls_config;

/// Owns one stable TLS watch channel. Input changes are prepared synchronously
/// under the config mutation lock before consumers are notified.
pub(super) struct RuntimeTls {
    sender: watch::Sender<TlsConfig>,
    inputs: Mutex<(Option<Arc<AcmeConfig<Arc<Path>>>>, Duration)>,
    fallback: TlsConfig,
    active_challenges: ActiveChallenges,
    config: Weak<DynConfig>,
}

impl RuntimeTls {
    fn new(
        config: &Arc<DynConfig>,
        root_ca: PrivateRootCa,
        active_challenges: ActiveChallenges,
    ) -> Self {
        let fallback =
            EitherConfig::Left(make_tls_config(&root_ca).expect("private root CA TLS config"));
        let this = Self {
            sender: watch::channel(fallback.clone()).0,
            inputs: Mutex::new((None, Duration::ZERO)),
            fallback,
            active_challenges,
            config: Arc::downgrade(config),
        };
        this.refresh(config);
        this
    }

    pub(super) fn refresh(&self, config: &DynConfig) {
        let acme = config.letsencrypt.get();
        let threshold = config.server.with(|s| s.certificate_renewal_threshold);
        let mut inputs = self.inputs.lock().unwrap();
        let unchanged = match (&inputs.0, &acme) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        if unchanged && inputs.1 == threshold {
            return;
        }
        let tls = if let Some(acme) = &acme {
            let weak = self.config.clone();
            let expected = acme.clone();
            let updates = DynamicAcmeConfig::from_update(move |new, key| {
                if let Some(config) = weak.upgrade() {
                    config.update_acme(&expected, new, key)
                } else {
                    Ok(false)
                }
            });
            EitherConfig::Right(SecurityConfig {
                trusted_store: NativeTrustedStoreConfig,
                certificate: AcmeCertificateConfig::new(
                    updates,
                    acme.clone().into(),
                    self.active_challenges.clone(),
                    threshold,
                ),
            })
        } else {
            self.fallback.clone()
        };
        *inputs = (acme, threshold);
        self.sender.send_replace(tls);
    }
}

impl DynConfig {
    pub(in crate::backend) fn initialize_tls(
        self: &Arc<Self>,
        root_ca: PrivateRootCa,
        active_challenges: ActiveChallenges,
    ) -> watch::Receiver<TlsConfig> {
        let _state = self.state.lock().unwrap();
        let runtime = RuntimeTls::new(self, root_ca, active_challenges);
        let receiver = runtime.sender.subscribe();
        assert!(self.tls.set(runtime).is_ok(), "TLS initialized twice");
        receiver
    }
}
