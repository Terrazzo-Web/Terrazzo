//! The Terrazzo Gateway [Client].

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;
use std::time::Instant;

use connect::ConnectError;
use futures::FutureExt as _;
use futures::future::BoxFuture;
use futures::future::Shared;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use tokio::sync::oneshot;
use tracing::Instrument;
use tracing::info;
use tracing::info_span;
use tracing::warn;
use trz_gateway_common::declare_identifier;
use trz_gateway_common::handle::ServerHandle;
use trz_gateway_common::id::ClientId;
use trz_gateway_common::id::ClientName;
use trz_gateway_common::retry_strategy::ProcessRetryStrategyHooks;
use trz_gateway_common::retry_strategy::RetryStrategy;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::security_configuration::certificate::tls_server::ToTlsServer as _;
use trz_gateway_common::security_configuration::certificate::tls_server::ToTlsServerError;
use trz_gateway_common::security_configuration::custom_server_certificate_verifier::ChainOnlyServerCertificateVerifier;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ToTlsClient as _;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ToTlsClientError;
use uuid::Uuid;

use self::config::ClientTransport;
use self::config::SniOverrideError;
use self::service::ClientService;
use crate::tunnel_config::TunnelConfig;

pub mod certificate;
pub mod config;
pub mod connect;
mod connection;
mod health;
pub mod service;
mod transport_stream;

/// The [Client].
///
/// It creates a WebSocket tunnel with the Terrazzo Gateway, and then runs a
/// gRPC server that listens to requests sent or forwarded by the Terrazzo
/// Gateway over the WebSocket tunnel.
pub struct Client {
    /// The client name for troubleshooting purposes
    pub client_name: ClientName,

    gateway_client: GatewayClient,
    client_api_server: ClientApiServer,
}

/// Configuration for how to create tunnels to the Terrazzo Gateway
struct GatewayClient {
    /// The URL used to open the TCP connection.
    gateway_uri: String,

    /// The TLS server name to validate when it differs from [Self::uri].
    gateway_sni_override: Option<String>,

    /// How the Gateway is reached before applying its TLS protocol.
    transport: ClientTransport,

    /// The TLS client is used to create the secure WebSocket tunnel.
    ///
    /// (without client certificate auth)
    gateway_tls_connector: tokio_tungstenite::Connector,
}

/// Configuration for the server running through tunnels to the Terrazzo Gateway.
struct ClientApiServer {
    /// The TLS server is used to accept gRPC connections through the tunnel.
    ///
    /// The client uses its certiticate to authenticate the server side of the connection.
    client_api_acceptor: tokio_rustls::TlsAcceptor,

    /// A callback to configure the [tonic gRPC server](tonic::transport::Server).
    client_service: Arc<dyn ClientService>,

    /// The strategy to retry creating the WebSocket tunnel when it fails
    retry_strategy: RetryStrategy,

    /// A global mutable variable that holds the [AuthCode].
    ///
    /// This is used to periodically renew the certificate.
    current_auth_code: Arc<Mutex<AuthCode>>,
}

declare_identifier!(AuthCode);

impl Client {
    /// Creates a new [Client].
    pub fn new<C: TunnelConfig>(config: C) -> Result<Arc<Self>, NewClientError<C>> {
        let tls_client = config
            .gateway_pki()
            .to_tls_client(ChainOnlyServerCertificateVerifier)?;
        let tls_server = config.client_certificate().to_tls_server()?;
        let client_name = config.client_name();
        let tunnel_path = format!("/remote/tunnel/{client_name}");
        Ok(Arc::new(Client {
            client_name,
            gateway_client: GatewayClient {
                gateway_uri: config.url(&tunnel_path)?.to_string(),
                gateway_sni_override: config.gateway_sni_override().map(ToOwned::to_owned),
                transport: config.transport(),
                gateway_tls_connector: tokio_tungstenite::Connector::Rustls(tls_client.into()),
            },
            client_api_server: ClientApiServer {
                client_api_acceptor: tokio_rustls::TlsAcceptor::from(tls_server),
                client_service: Arc::new(config.client_service()),
                retry_strategy: config.retry_strategy(),
                current_auth_code: config.current_auth_code(),
            },
        }))
    }

    /// Runs the client and returns a handle to stop the client.
    pub async fn run(self: &Arc<Self>) -> Result<ServerHandle<()>, ConnectError> {
        let this = self.clone();
        let client_name = &this.client_name;
        let span = info_span!("Run", %client_name);
        async move {
            let client_id = ClientId::from(Uuid::new_v4().to_string());
            info!(%client_id, "Allocated new client id");
            let (shutdown_rx, terminated_tx, handle) = ServerHandle::new("Client");
            let (serving_tx, serving_rx) = oneshot::channel();
            let task = run_impl(this, client_id, serving_tx, shutdown_rx, terminated_tx);
            tokio::spawn(task.in_current_span());
            let _ = serving_rx.await;
            Ok(handle)
        }
        .instrument(span)
        .await
    }
}

async fn run_impl(
    this: Arc<Client>,
    client_id: ClientId,

    // Set when the client is serving connections
    serving_tx: oneshot::Sender<()>,

    // Set when the client should start shutting down
    shutdown_rx: impl Future<Output = ()> + Send + 'static,

    // Set when the client has shut down
    terminated_tx: oneshot::Sender<()>,
) {
    scopeguard::defer! { let _ = terminated_tx.send(()); };
    let retry_strategy = this.client_api_server.retry_strategy.clone();
    let shutdown_rx: BoxFuture<()> = Box::pin(shutdown_rx);
    let shutdown_rx = shutdown_rx.shared();

    // TODO: Use retry_strategy.process instead of process2, ConnectRetryHooks is not required, instead move the shutdown logic outside of retry_strategy.process, use futures::future::select instead of tokio's select! macro, use autoclone instead of manually cloning references into the process callback. validate with bazel test //...
    let is_shutdown = is_shutdown(shutdown_rx.clone());
    let serving_tx = Arc::new(tokio::sync::Mutex::new(Some(serving_tx)));
    let attempt = Arc::new(AtomicUsize::new(0));
    let connect_retry_delay = Arc::new(Mutex::new(retry_strategy.peek() / 2));
    let reset_retry_delay = retry_strategy.peek() / 2;
    retry_strategy
        .process2(
            || {
                let this = this.clone();
                let client_id = client_id.clone();
                let shutdown_rx = shutdown_rx.clone();
                let is_shutdown = is_shutdown.clone();
                let serving_tx = serving_tx.clone();
                let connect_retry_delay = connect_retry_delay.clone();
                let attempt = attempt.fetch_add(1, SeqCst);
                async move {
                    if is_shutdown.load(SeqCst) || shutdown_rx.clone().now_or_never().is_some() {
                        return ControlFlow::Break(());
                    }
                    let retry_delay = *connect_retry_delay.lock().unwrap();
                    let mut serving_tx = serving_tx.lock().await;
                    let result = this
                        .connect(client_id, shutdown_rx.clone(), retry_delay, &mut serving_tx)
                        .instrument(info_span!("Connect", attempt))
                        .await;
                    if is_shutdown.load(SeqCst) || shutdown_rx.now_or_never().is_some() {
                        return ControlFlow::Break(());
                    }
                    ControlFlow::Continue(match result {
                        Ok(()) => ConnectRetryError::Closed,
                        Err(error) => ConnectRetryError::Failed(error),
                    })
                }
            },
            ConnectRetryHooks {
                shutdown_rx: shutdown_rx.clone(),
                connect_retry_delay: connect_retry_delay.clone(),
                reset_retry_delay,
            },
        )
        .await;
}

#[derive(Debug, thiserror::Error)]
enum ConnectRetryError {
    #[error("Connection closed")]
    Closed,
    #[error("Connection failed: {0}")]
    Failed(ConnectError),
}

struct ConnectRetryHooks {
    shutdown_rx: Shared<BoxFuture<'static, ()>>,
    connect_retry_delay: Arc<Mutex<Duration>>,
    reset_retry_delay: Duration,
}

impl ProcessRetryStrategyHooks for ConnectRetryHooks {
    type Error = ConnectRetryError;

    fn on_start(&mut self, _now: Instant) {}

    fn on_reset(&mut self, _elapsed: Duration, error: Self::Error) {
        *self.connect_retry_delay.lock().unwrap() = self.reset_retry_delay;
        match error {
            ConnectRetryError::Closed => info!("Connection closed; retry strategy reset"),
            ConnectRetryError::Failed(error) => {
                warn!(%error, "Connection failed; retry strategy reset")
            }
        }
    }

    fn on_error(&mut self, _elapsed: Duration, waiting: Duration, error: Self::Error) {
        match error {
            ConnectRetryError::Closed => {
                info!(retry = %humantime::format_duration(waiting), "Connection closed; retrying")
            }
            ConnectRetryError::Failed(error) => {
                warn!(retry = %humantime::format_duration(waiting), %error, "Connection failed; retrying")
            }
        }
    }

    fn wait(&mut self, current: &mut RetryStrategy) -> impl Future<Output = ()> {
        let delay = current.wait();
        *self.connect_retry_delay.lock().unwrap() = current.peek() / 2;
        let shutdown_rx = self.shutdown_rx.clone();
        async move {
            tokio::select! {
                () = delay => {}
                () = shutdown_rx => {}
            }
        }
    }
}

fn is_shutdown(shutdown_rx: Shared<impl Future<Output = ()> + Send + 'static>) -> Arc<AtomicBool> {
    let is_shutdown = Arc::new(AtomicBool::new(false));
    tokio::spawn({
        let is_shutdown = is_shutdown.clone();
        async move {
            let _ = shutdown_rx.await;
            is_shutdown.store(true, SeqCst);
        }
    });
    return is_shutdown;
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum NewClientError<C: TunnelConfig> {
    #[error("[{n}] {0}", n = self.name())]
    SniOverride(#[from] SniOverrideError),

    #[error("[{n}] {0}", n = self.name())]
    ToTlsClient(#[from] ToTlsClientError<<C::GatewayPki as TrustedStoreConfig>::Error>),

    #[error("[{n}] {0}", n = self.name())]
    ToTlsServer(#[from] ToTlsServerError<<C::ClientCertificate as CertificateConfig>::Error>),
}
