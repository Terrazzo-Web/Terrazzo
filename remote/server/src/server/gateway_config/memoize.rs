use std::sync::Mutex;
use std::sync::OnceLock;

use super::GatewayConfig;
use super::app_config::AppConfig;

pub struct MemoizedGatewayConfig<C: GatewayConfig> {
    enable_tracing: bool,
    gateway_config: OnceLock<C>,
    make_config: Mutex<Option<Box<dyn FnOnce() -> C + Send>>>,
}

impl<C: GatewayConfig> MemoizedGatewayConfig<C> {
    pub fn new<E: std::error::Error>(
        enable_tracing: bool,
        f: impl FnOnce() -> Result<C, E> + Send + 'static,
    ) -> Self {
        Self {
            enable_tracing,
            gateway_config: OnceLock::new(),
            make_config: Mutex::new(Some(Box::new(|| match f() {
                Ok(config) => config,
                Err(error) => panic!("Failed to load gateway config: {error}"),
            }))),
        }
    }
}

impl<C: GatewayConfig + Clone> GatewayConfig for MemoizedGatewayConfig<C> {
    fn enable_tracing(&self) -> bool {
        self.enable_tracing
    }
    fn host(&self) -> &str {
        self.load().host()
    }
    fn port(&self) -> u16 {
        self.load().port()
    }

    type RootCaConfig = C::RootCaConfig;
    fn root_ca(&self) -> Self::RootCaConfig {
        self.load().root_ca()
    }

    type TlsConfig = C::TlsConfig;
    fn tls(&self) -> Self::TlsConfig {
        self.load().tls()
    }

    type ClientCertificateIssuerConfig = C::ClientCertificateIssuerConfig;
    fn client_certificate_issuer(&self) -> Self::ClientCertificateIssuerConfig {
        self.load().client_certificate_issuer()
    }

    fn app_config(&self) -> impl AppConfig {
        self.load().app_config()
    }
}

impl<C: GatewayConfig + Clone> MemoizedGatewayConfig<C> {
    fn load(&self) -> &C {
        self.gateway_config.get_or_init(|| {
            let make_config = self.make_config.lock().unwrap().take();
            let make_config = make_config.unwrap();
            return make_config();
        })
    }
}
