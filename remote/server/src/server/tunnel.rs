use std::io::ErrorKind;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use axum::extract::Path;
use axum::extract::WebSocketUpgrade;
use axum::response::IntoResponse;
use futures::SinkExt;
use futures::StreamExt;
use rustls::ClientConfig;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio_rustls::rustls;
use tokio_rustls::rustls::client::WebPkiServerVerifier;
use tokio_rustls::rustls::pki_types::DnsName;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::RootCertStore;
use tokio_rustls::TlsConnector;
use tokio_util::io::CopyToBytes;
use tracing::info;
use tracing::info_span;
use tracing::Instrument as _;
use trz_gateway_common::declare_identifier;

use super::gateway_configuration::GatewayConfig;
use super::Server;

declare_identifier!(ClientId);

impl<C: GatewayConfig> Server<C> {
    pub async fn tunnel(
        self: Arc<Self>,
        Path(client_id): Path<ClientId>,
        web_socket: WebSocketUpgrade,
    ) -> impl IntoResponse {
        let span = info_span!("Tunnel", %client_id);
        let _entered = span.clone().entered();
        info!("Incoming tunnel");
        web_socket.on_upgrade(move |web_socket| {
            self.process_websocket(client_id, web_socket)
                .instrument(span)
        })
    }

    async fn process_websocket(self: Arc<Self>, client_id: ClientId, web_socket: WebSocket) {
        let (sink, stream) = web_socket.split();
        let reader = tokio_util::io::StreamReader::new(stream.map(|message| match message {
            Ok(message) => Ok(message.into_data()),
            Err(error) => Err(std::io::Error::new(
                ErrorKind::ConnectionAborted,
                error.into_inner(),
            )),
        }));
        let writer = tokio_util::io::SinkWriter::new(
            CopyToBytes::new(
                sink.with(|data: Bytes| std::future::ready(Ok(Message::Binary(data)))),
            )
            .sink_map_err(|error: axum::Error| {
                std::io::Error::new(ErrorKind::ConnectionAborted, error.into_inner())
            }),
        );
        tokio::spawn(self.process_connection(client_id, tokio::io::join(reader, writer)));
    }

    async fn process_connection(
        self: Arc<Self>,
        client_id: ClientId,
        connection: impl AsyncRead + AsyncWrite + Unpin,
    ) {
        let mut roots = RootCertStore::empty();
        roots
            .add(self.root_ca.certificate.to_der().unwrap().into())
            .unwrap();
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(
                WebPkiServerVerifier::builder(roots.into()).build().unwrap(),
            )
            .with_no_client_auth();
        let config = TlsConnector::from(Arc::new(config));

        let _tls_stream = config
            .connect(
                ServerName::DnsName(DnsName::try_from(client_id.to_string()).unwrap()),
                connection,
            )
            .await
            .unwrap();
    }
}
