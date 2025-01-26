use std::sync::Arc;

use crate::security_configuration::SecurityConfig;
use crate::utils::is_configuration::IsConfiguration;

pub trait GatewayConfig: IsConfiguration {
    fn enable_tracing(&self) -> bool {
        true
    }

    fn host(&self) -> &str {
        "127.0.0.1"
    }

    fn port(&self) -> u16 {
        if cfg!(debug_assertions) {
            3000
        } else {
            3001
        }
    }

    type RootCaConfig: SecurityConfig;
    fn root_ca(&self) -> Self::RootCaConfig;

    type TlsConfig: SecurityConfig;
    fn tls(&self) -> Self::TlsConfig;
}

impl<T: GatewayConfig> GatewayConfig for Arc<T> {
    fn enable_tracing(&self) -> bool {
        let this: &T = self.as_ref();
        this.enable_tracing()
    }
    fn host(&self) -> &str {
        let this: &T = self.as_ref();
        this.host()
    }
    fn port(&self) -> u16 {
        let this: &T = self.as_ref();
        this.port()
    }

    type RootCaConfig = T::RootCaConfig;
    fn root_ca(&self) -> Self::RootCaConfig {
        let this: &T = self.as_ref();
        this.root_ca()
    }

    type TlsConfig = T::TlsConfig;
    fn tls(&self) -> Self::TlsConfig {
        let this: &T = self.as_ref();
        this.tls()
    }
}
