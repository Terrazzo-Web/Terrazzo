use std::future::ready;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use futures::FutureExt as _;
use futures::StreamExt as _;
use futures::future::BoxFuture;
use futures::future::Either;
use futures::future::Shared;
use http::header::InvalidHeaderValue;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use reqwest::Url;
use scopeguard::defer;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;
use tokio::time::error::Elapsed;
use tokio_tungstenite::tungstenite;
use tonic::transport::Server;
use tracing::Span;
use tracing::debug;
use tracing::info;
use tracing::info_span;
use tracing::warn;
use tracing_futures::Instrument as _;
use trz_gateway_common::id::CLIENT_ID_HEADER;
use trz_gateway_common::id::ClientId;
use trz_gateway_common::protos::terrazzo::remote::health::health_service_server::HealthServiceServer;
use trz_gateway_common::to_async_io::WebSocketIo;

use self::tungstenite::client::IntoClientRequest as _;
use super::config::ClientTransport;
use super::config::SniOverrideError;
use super::config::set_sni_override;
use super::connection::Connection;
use super::connection::ForceCloseIo;
use super::health::HealthServiceImpl;

impl super::Client {
    /// API to create tunnels to the Terrazzo Gateway.
    pub(super) async fn connect(
        &self,
        client_id: ClientId,
        shutdown: Shared<BoxFuture<'static, ()>>,
        timeout: Duration,
        serving: &mut Option<oneshot::Sender<()>>,
    ) -> Result<(), ConnectError> {
        let start = Instant::now();
        info!(uri = self.uri, sni = ?self.sni_override, transport = ?self.transport, "Connecting WebSocket");
        let web_socket_config = None;
        let disable_nagle = true;

        let request = format!("ws{}", &self.uri["http".len()..])
            .into_client_request()
            .map_err(Box::from)?;
        let mut tls_request = websocket_url(&self.uri, self.sni_override.as_deref())?
            .as_str()
            .into_client_request()
            .map_err(Box::from)?;
        tls_request
            .headers_mut()
            .append(&CLIENT_ID_HEADER, client_id.as_ref().try_into()?);
        let socket = connect_transport(&self.transport, &request, disable_nagle, timeout)
            .instrument(info_span!("Connect Transport"))
            .await?;
        let (socket, force_close) = ForceCloseIo::new(socket);
        let (web_socket, response) = tokio_tungstenite::client_async_tls_with_config(
            tls_request,
            socket,
            web_socket_config,
            Some(self.tls_client.clone()),
        )
        .timeout(timeout)
        .await
        .map_err(|_: Elapsed| ConnectError::Timeout("WebSocket"))?
        .map_err(Box::from)?;
        info!("Connected WebSocket");
        debug!("WebSocket response: {response:?}");

        let (stream, eos) = TungsteniteWebSocketIo::to_async_io(web_socket);
        let eos = eos.map(|r| r.map_err(Arc::new)).shared();
        let tls_stream = self
            .tls_server
            .accept(stream)
            .timeout(timeout)
            .await
            .map_err(|_: Elapsed| ConnectError::Timeout("TLS handshake"))?
            .map_err(ConnectError::Accept)?;

        let connection = Connection::new(tls_stream);
        let (unhealthy_tx, unhealthy_rx) = oneshot::channel();
        let unhealthy_rx = unhealthy_rx.shared();
        let eos2 = futures::future::select(eos.clone(), unhealthy_rx.clone());
        let incoming = futures::stream::once(ready(Ok(connection)))
            .chain(futures::stream::once(async move {
                match eos2.await {
                    Either::Left((eos, unhealthy_rx)) => {
                        handle_close_timeout("EOS", eos, "Unhealthy", unhealthy_rx)
                    }
                    Either::Right((unhealthy_rx, eos)) => {
                        handle_close_timeout("Unhealthy", unhealthy_rx, "EOS", eos)
                    }
                }
                Err(ConnectError::Disconnected)
            }))
            .in_current_span();

        let current_span = Span::current();
        let grpc_server = self
            .client_service
            .configure_service(
                Server::builder()
                    .tcp_keepalive(None)
                    .tcp_nodelay(true)
                    .http2_keepalive_interval(None)
                    .http2_keepalive_timeout(None)
                    .trace_fn(move |_| current_span.clone()),
            )
            .add_service(HealthServiceServer::new(HealthServiceImpl::new(
                self.current_auth_code.clone(),
                unhealthy_tx,
                shutdown.clone(),
            )));

        info!(
            elapsed = humantime::format_duration(start.elapsed()).to_string(),
            "Serving"
        );

        // Signal first time client is ready to serve.
        serving.take().map(|serving| serving.send(()));

        let shutdown = futures::future::select(shutdown, unhealthy_rx)
            .map(move |signal| {
                match signal {
                    Either::Left(((), _)) => info!("Shutdown signal"),
                    Either::Right((Ok(()), _)) => info!("Unhealthy signal"),
                    Either::Right((Err(RecvError { .. }), _)) => {
                        warn!("Unhealthy signal dropped")
                    }
                }
                force_close.close();
            })
            .in_current_span();
        let () = grpc_server
            .serve_with_incoming_shutdown(incoming, shutdown)
            .await?;
        debug!("Waiting for EOS");
        if let Some(eos) = eos.peek().cloned() {
            let () = eos.map_err(ConnectError::Stream)?;
        }
        info!(
            elapsed = humantime::format_duration(start.elapsed()).to_string(),
            "Done"
        );
        Ok(())
    }
}

fn handle_close_timeout<E1: std::error::Error, E2: std::error::Error>(
    result_is: &'static str,
    result: Result<(), E1>,
    pending_is: &'static str,
    pending: impl Future<Output = Result<(), E2>> + Send + Clone + 'static,
) {
    match result {
        Ok(()) => warn!("Stream closed with {result_is}:OK"),
        Err(error) => warn!("Stream closed with {result_is}:{error}"),
    }
    let close_latency = {
        let start = Instant::now();
        move || humantime::format_duration(start.elapsed())
    };
    tokio::spawn(
        async move {
            const MINUTE: Duration = Duration::from_secs(60 * 5);
            match tokio::time::timeout(MINUTE, pending).await {
                Ok(Ok(())) => {
                    info!("{pending_is} triggered after {}", close_latency())
                }
                Ok(Err(error)) => {
                    warn!(
                        "{pending_is} triggered after {} with {error}",
                        close_latency()
                    )
                }
                Err(tokio::time::error::Elapsed { .. }) => {
                    warn!("{pending_is} never triggered until {}", close_latency())
                }
            }
        }
        .in_current_span(),
    );
}

trait HasTimeout: Future + Sized {
    fn timeout(
        self,
        duration: Duration,
    ) -> impl Future<Output = Result<<Self as Future>::Output, Elapsed>> {
        tokio::time::timeout(duration, self)
    }
}

impl<T: Future + Sized> HasTimeout for T {}

fn websocket_url(uri: &str, sni_override: Option<&str>) -> Result<Url, SniOverrideError> {
    let mut url = Url::parse(&format!("ws{}", &uri["http".len()..]))?;
    set_sni_override(&mut url, sni_override)?;
    Ok(url)
}

async fn connect_tcp(
    request: &tungstenite::handshake::client::Request,
    disable_nagle: bool,
) -> Result<TcpStream, ConnectError> {
    let host = request
        .uri()
        .host()
        .ok_or(ConnectError::MissingEndpointHost)?;
    let port = request
        .uri()
        .port_u16()
        .or_else(|| match request.uri().scheme_str() {
            Some("wss") => Some(443),
            Some("ws") => Some(80),
            _ => None,
        })
        .ok_or(ConnectError::MissingEndpointPort)?;
    let socket = TcpStream::connect((host, port))
        .await
        .map_err(ConnectError::TcpConnect)?;
    if disable_nagle {
        socket.set_nodelay(true).map_err(ConnectError::TcpConnect)?;
    }
    Ok(socket)
}

trait TransportIo: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T> TransportIo for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

async fn connect_transport(
    transport: &ClientTransport,
    request: &tungstenite::handshake::client::Request,
    disable_nagle: bool,
    timeout: Duration,
) -> Result<Box<dyn TransportIo>, ConnectError> {
    let start = Instant::now();
    info!("Start");
    defer!(info!(elapsed = %humantime::format_duration(start.elapsed()), "End"));
    match transport {
        ClientTransport::Direct => Ok(Box::new(
            connect_tcp(request, disable_nagle)
                .timeout(timeout)
                .await
                .map_err(|_: Elapsed| ConnectError::Timeout("TCP connect"))??,
        )),
        ClientTransport::WebRtc(config) => Ok(Box::new(crate::p2p::connect(config).await?)),
    }
}

/// Errors returned by [Client::run](super::Client::run).
#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ConnectError {
    #[error("[{n}] {0}", n = self.name())]
    InvalidHeader(#[from] InvalidHeaderValue),

    #[error("[{n}] {0}", n = self.name())]
    Connect(#[from] Box<tungstenite::Error>),

    #[error("[{n}] {0}", n = self.name())]
    SniOverride(#[from] SniOverrideError),

    #[error("[{n}] The Gateway endpoint must include a host", n = self.name())]
    MissingEndpointHost,

    #[error("[{n}] The Gateway endpoint must include or imply a port", n = self.name())]
    MissingEndpointPort,

    #[error("[{n}] {0}", n = self.name())]
    TcpConnect(std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    P2p(#[from] crate::p2p::P2pConnectError),

    #[error("[{n}] {0}", n = self.name())]
    Accept(std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    Tunnel(#[from] tonic::transport::Error),

    #[error("[{n}] {0}", n = self.name())]
    Stream(Arc<std::io::Error>),

    #[error("[{n}] The client got disconnected", n = self.name())]
    Disconnected,

    #[error("[{n}] {0}", n = self.name())]
    Timeout(&'static str),
}

struct TungsteniteWebSocketIo;
impl WebSocketIo for TungsteniteWebSocketIo {
    type Message = tungstenite::Message;
    type Error = tungstenite::Error;

    fn into_data(message: Self::Message) -> tungstenite::Bytes {
        message.into_data()
    }

    fn into_messsge(bytes: tungstenite::Bytes) -> Self::Message {
        tungstenite::Message::Binary(bytes)
    }
}
