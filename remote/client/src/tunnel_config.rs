//! Configuration for the Terrazzo tunnel.

use std::sync::Arc;
use std::sync::Mutex;

use trz_gateway_common::retry_strategy::RetryStrategy;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;

use super::client::AuthCode;
use super::client::config::ClientConfig;
use super::client::service::ClientService;

/// Configuration for the Terrazzo tunnel.
///
/// - Parent [ClientConfig] specifies the endpoint and PKI of the Gateway
/// - This [TunnelConfig] provides the client certificate and the gRPC service
///   to use to communicate over the tunnel.
pub trait TunnelConfig: ClientConfig {
    /// The TLS certificate issued by the Terrazzo Gateway.
    type ClientCertificate: CertificateConfig;
    fn client_certificate(&self) -> Self::ClientCertificate;

    /// Returns a [ClientService] to configure the gRPC server running in the client.
    fn client_service(&self) -> impl ClientService;

    /// The retry strategy for reconnecting to the Terrazzo Gateway on connection failure.
    fn retry_strategy(&self) -> RetryStrategy;

    /// A reference to the current [AuthCode], kept up-to-date by the keep-alive.
    ///
    /// This is used to renew the certificate before expiration.
    fn current_auth_code(&self) -> Arc<Mutex<AuthCode>>;
}

impl<T: TunnelConfig> TunnelConfig for Arc<T> {
    type ClientCertificate = T::ClientCertificate;
    fn client_certificate(&self) -> Self::ClientCertificate {
        self.as_ref().client_certificate()
    }

    fn client_service(&self) -> impl ClientService {
        self.as_ref().client_service()
    }

    fn retry_strategy(&self) -> RetryStrategy {
        self.as_ref().retry_strategy()
    }

    fn current_auth_code(&self) -> Arc<Mutex<AuthCode>> {
        self.as_ref().current_auth_code()
    }
}
