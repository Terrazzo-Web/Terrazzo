use std::sync::Mutex;
use std::sync::OnceLock;

use super::GatewayConfig;
use super::app_config::AppConfig;

pub(super) struct MemoizedGatewayConfig<C: GatewayConfig> {
    gateway_config: OnceLock<C>,
    make_config: Mutex<Option<Box<dyn FnOnce() -> C + Send>>>,
}

impl<C: GatewayConfig + Clone> GatewayConfig for MemoizedGatewayConfig<C> {
    fn enable_tracing(&self) -> bool {
        self.load().enable_tracing()
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
            let maybe_f = self.make_config.lock().unwrap().take();
            let f = maybe_f.unwrap();
            return f();
        })
    }
}
