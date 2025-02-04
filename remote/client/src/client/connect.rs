use std::future::ready;
use std::io::ErrorKind;
use std::sync::Arc;

use futures::SinkExt as _;
use futures::StreamExt as _;
use nameth::nameth;
use nameth::NamedEnumValues as _;
use nameth::NamedType as _;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::io::CopyToBytes;
use tokio_util::io::SinkWriter;
use tokio_util::io::StreamReader;
use tracing::debug;
use tracing::info;

use trz_gateway_common::security_configuration::certificate::tls_server::ToTlsServer;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ChainOnlyServerCertificateVerifier;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ToTlsClient;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ToTlsClientError;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;

use super::Client;
use crate::client_configuration::ClientConfig;

impl Client {
    pub async fn connect<C: ClientConfig>(&self, client_config: C) -> Result<(), ConnectError<C>> {
        let request = format!(
            "{}/remote/tunnel/{}",
            client_config.base_url(),
            client_config.client_id()
        );
        info!("Connecting WebSocket to {request}");
        let web_socket_config = None;
        let disable_nagle = true;
        let tls_client = client_config
            .server_pki()
            .to_tls_client(ChainOnlyServerCertificateVerifier)
            .await?;
        let (web_socket, response) = tokio_tungstenite::connect_async_tls_with_config(
            request,
            web_socket_config,
            disable_nagle,
            Some(tokio_tungstenite::Connector::Rustls(tls_client.into())),
        )
        .await?;
        info!("Connected WebSocket");
        debug!("WebSocket response: {response:?}");

        let (sink, stream) = web_socket.split();

        let reader = {
            #[nameth]
            #[derive(thiserror::Error, Debug)]
            #[error("[{n}] {0}", n = Self::type_name())]
            struct ReadError(tungstenite::Error);

            StreamReader::new(stream.map(|message| {
                message.map(Message::into_data).map_err(|error| {
                    std::io::Error::new(ErrorKind::ConnectionAborted, ReadError(error))
                })
            }))
        };

        let writer = {
            #[nameth]
            #[derive(thiserror::Error, Debug)]
            #[error("[{n}] {0}", n = Self::type_name())]
            struct WriteError(tungstenite::Error);

            let sink = CopyToBytes::new(sink.with(|data| ready(Ok(Message::Binary(data)))))
                .sink_map_err(|error| {
                    std::io::Error::new(ErrorKind::ConnectionAborted, WriteError(error))
                });
            SinkWriter::new(sink)
        };

        let stream = tokio::io::join(reader, writer);

        let tls_server = client_config.tls().to_tls_server().await.unwrap();
        let tls_server = TlsAcceptor::from(Arc::from(tls_server));
        let tls_stream = tls_server.accept(stream);
        // TODO:
        // 1. do TLS with client certificate
        // 1. do gRPC on top
        Ok(())
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ConnectError<C: ClientConfig> {
    #[error("[{n}] {0}", n = self.name())]
    ToTlsClient(#[from] ToTlsClientError<<C::ServerPkiConfig as TrustedStoreConfig>::Error>),

    #[error("[{n}] {0}", n = self.name())]
    Connect(#[from] tokio_tungstenite::tungstenite::Error),
}
