use std::sync::Arc;
use std::time::Instant;

use futures::FutureExt as _;
use futures::SinkExt as _;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use hyper_util::rt::TokioTimer;
use hyper_util::server::conn::auto;
use hyper_util::service::TowerToHyperService;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async_with_config;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tracing::Instrument as _;
use tracing::info;
use tracing::info_span;
use tracing::warn;
use trz_gateway_common::p2p::protocol::MAX_SDP_LEN;
use trz_gateway_common::p2p::protocol::PROTOCOL_VERSION;
use trz_gateway_common::p2p::protocol::SignalMessage;
use url::Url;

use self::registration::Registration;
use self::stream::P2pServerStream;
use super::error::P2pServerError;
use crate::server::HTTP_TIMEOUT;
use crate::server::Server;
use crate::server::gateway_config::p2p::P2pRegistrationAuthorization;
use crate::server::gateway_config::p2p::P2pRegistrationConfig;

mod registration;
pub mod stream;

const REGISTRATION_QUEUE_CAPACITY: usize = 128;
const SESSION_QUEUE_CAPACITY: usize = 64;
const MAX_SIGNAL_MESSAGE_SIZE: usize = MAX_SDP_LEN + 64 * 1024;

impl Server {
    pub(in crate::server) fn start_p2p_registration(
        self: &Arc<Self>,
        config: P2pRegistrationConfig,
    ) {
        let server = self.clone();
        let server_name = config.server_name.clone();
        tokio::spawn(
            start_p2p_registration_impl(config, server)
                .instrument(info_span!("P2pServer", %server_name)),
        );
    }

    async fn run_p2p_registration(
        self: Arc<Self>,
        config: P2pRegistrationConfig,
    ) -> Result<(), P2pServerError> {
        if config.max_sessions == 0 {
            return Err(P2pServerError::InvalidConfig(
                "max_sessions must be greater than zero".into(),
            ));
        }
        let request = registration_request(&config)?;
        let websocket_config = tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(MAX_SIGNAL_MESSAGE_SIZE))
            .max_frame_size(Some(MAX_SIGNAL_MESSAGE_SIZE));
        let (mut socket, _) =
            connect_async_with_config(request, Some(websocket_config), false).await?;
        send_signal(
            &mut socket,
            &SignalMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
            },
        )
        .await?;
        info!("Registered outbound signaling WebSocket");

        Registration::new(self, config).run(socket).await
    }

    /// Serves the gateway's existing TLS and Axum stack on one reliable channel.
    pub(super) async fn serve_p2p_connection(
        self: Arc<Self>,
        connection: P2pServerStream,
    ) -> Result<(), P2pServerError> {
        let tls = self.p2p_tls_server.accept(connection).await?;
        let service = TowerToHyperService::new(self.make_app());
        let mut builder = auto::Builder::new(TokioExecutor::new());
        builder
            .http1()
            .timer(TokioTimer::new())
            .header_read_timeout(HTTP_TIMEOUT);
        builder
            .http2()
            .timer(TokioTimer::new())
            .keep_alive_timeout(HTTP_TIMEOUT);
        builder
            .serve_connection_with_upgrades(TokioIo::new(tls), service)
            .await
            .map_err(P2pServerError::ServeHttp)
    }
}

async fn start_p2p_registration_impl(config: P2pRegistrationConfig, server: Arc<Server>) {
    let mut retry = config.retry_strategy.clone();
    loop {
        let started = Instant::now();
        let result = server.clone().run_p2p_registration(config.clone()).await;
        if server.shutdown.clone().now_or_never().is_some() {
            return;
        }
        match result {
            Ok(()) => info!("P2P signaling registration closed"),
            Err(error) => warn!(%error, "P2P signaling registration failed"),
        }
        if started.elapsed() >= config.retry_strategy.max_delay() {
            retry = config.retry_strategy.clone();
        }
        let delay = retry.wait();
        tokio::select! {
            () = delay => {}
            () = server.shutdown.clone() => return,
        }
    }
}

fn registration_request(
    config: &P2pRegistrationConfig,
) -> Result<tungstenite::http::Request<()>, P2pServerError> {
    let mut url = Url::parse(&config.signaling_url)?;
    match url.scheme() {
        "http" => url
            .set_scheme("ws")
            .map_err(|_| P2pServerError::InvalidUrlScheme)?,
        "https" => url
            .set_scheme("wss")
            .map_err(|_| P2pServerError::InvalidUrlScheme)?,
        "ws" | "wss" => {}
        _ => return Err(P2pServerError::InvalidUrlScheme),
    }
    url.path_segments_mut()
        .map_err(|_| P2pServerError::InvalidUrlScheme)?
        .pop_if_empty()
        .extend(["p2p", "register", config.server_name.as_ref()]);
    let mut request = url.as_str().into_client_request()?;
    if let Some(P2pRegistrationAuthorization::BearerToken(token)) = &config.authorization {
        request.headers_mut().insert(
            tungstenite::http::header::AUTHORIZATION,
            tungstenite::http::HeaderValue::from_str(&format!("Bearer {}", token.as_string()))?,
        );
    }
    Ok(request)
}

async fn send_signal<S>(
    socket: &mut WebSocketStream<S>,
    message: &SignalMessage,
) -> Result<(), P2pServerError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    socket
        .send(Message::Text(serde_json::to_string(message)?.into()))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_encoded_registration_url_and_redacted_authorization() {
        let mut config = P2pRegistrationConfig::new(
            "https://signal.example/base/",
            "server name/with slash".into(),
        );
        config.authorization = Some(P2pRegistrationAuthorization::BearerToken("secret".into()));
        let request = registration_request(&config).unwrap();
        assert_eq!(
            "wss://signal.example/base/p2p/register/server%20name%2Fwith%20slash",
            request.uri().to_string(),
        );
        assert_eq!("Bearer secret", request.headers()["authorization"]);
    }
}
