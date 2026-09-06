use std::sync::Arc;

use bytes::Bytes;
use http::StatusCode;
use http::header::CONTENT_TYPE;
use http_body_util::BodyExt as _;
use http_body_util::Full;
use hyper::Request;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use reqwest::Certificate;
use rustls::ClientConfig as RustlsClientConfig;
use rustls::pki_types::ServerName;
use tokio_rustls::TlsConnector;
use tracing::debug;
use trz_gateway_common::security_configuration::custom_server_certificate_verifier::ChainOnlyServerCertificateVerifier;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ToTlsClient as _;
use trz_gateway_common::security_configuration::trusted_store::tls_client::ToTlsClientError;

use crate::client::config::ClientConfig;
use crate::client::config::ClientTransport;
use crate::client::config::P2pClientConfig;
use crate::client::config::SniOverrideError;
use crate::client::config::sni_override_resolution;

pub(super) enum HttpClient {
    Direct(reqwest::Client),
    P2p(P2pHttpClient),
}

pub(super) struct HttpResponse {
    pub status: StatusCode,
    pub body: String,
}

impl HttpClient {
    pub async fn get(
        self,
        url: reqwest::Url,
        content_type: &'static str,
        body: String,
    ) -> Result<HttpResponse, HttpRequestError> {
        match self {
            Self::Direct(client) => {
                let request = client
                    .get(url)
                    .header(CONTENT_TYPE, content_type)
                    .body(body);
                let response = request.send().await?;
                Ok(HttpResponse {
                    status: response.status(),
                    body: response.text().await?,
                })
            }
            Self::P2p(client) => client.get(url, content_type, body).await,
        }
    }
}

pub(super) fn make_http_client<C>(
    client_config: &C,
) -> Result<HttpClient, MakeHttpClientError<<C::GatewayPki as TrustedStoreConfig>::Error>>
where
    C: ClientConfig,
{
    // TLS trust by transport:
    // - Both authenticate the target Gateway hostname, including any SNI override.
    // - Both accept Gateway certificates issued by `gateway_pki`.
    // - Direct uses reqwest's platform verifier with `gateway_pki` as extra roots, so it also
    //   retains the operating system's roots.
    // - WebRTC's inner TLS trusts only `gateway_pki`.
    // - WebRTC signaling WSS is authenticated separately with platform roots.
    // - WebRTC DTLS protects the peer channel but does not replace the inner Gateway TLS.
    match client_config.transport() {
        ClientTransport::Direct => {
            debug!("Making a Direct client connection");
            make_direct_http_client(client_config).map(HttpClient::Direct)
        }
        ClientTransport::WebRtc(config) => {
            debug!("Making a WebRtc client connection: {config:#?}");
            let mut tls = client_config
                .gateway_pki()
                .to_tls_client(ChainOnlyServerCertificateVerifier)
                .map_err(MakeHttpClientError::ToTlsClient)?;
            tls.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
            Ok(HttpClient::P2p(P2pHttpClient {
                config,
                tls: Arc::new(tls),
            }))
        }
    }
}

fn make_direct_http_client<C>(
    client_config: &C,
) -> Result<reqwest::Client, MakeHttpClientError<<C::GatewayPki as TrustedStoreConfig>::Error>>
where
    C: ClientConfig,
{
    let mut builder = reqwest::Client::builder();
    let roots = client_config
        .gateway_pki()
        .root_certificates()
        .map_err(MakeHttpClientError::RootCertificates)?;
    for root in roots.all_certificates() {
        let root_der = root.to_der().map_err(MakeHttpClientError::RootToDer)?;
        let root_certificate =
            Certificate::from_der(&root_der).map_err(MakeHttpClientError::DerToCertificate)?;
        builder = builder.add_root_certificate(root_certificate);
    }
    if let Some((sni_override, socket_addr)) =
        sni_override_resolution(client_config).map_err(MakeHttpClientError::SniOverride)?
    {
        builder = builder.resolve(&sni_override, socket_addr);
    }
    builder.build().map_err(MakeHttpClientError::Build)
}

pub(super) struct P2pHttpClient {
    config: P2pClientConfig,
    tls: Arc<RustlsClientConfig>,
}

impl P2pHttpClient {
    async fn get(
        self,
        url: reqwest::Url,
        content_type: &'static str,
        body: String,
    ) -> Result<HttpResponse, HttpRequestError> {
        let timeout = self.config.connect_timeout;
        tokio::time::timeout(timeout, self.get_inner(url, content_type, body))
            .await
            .map_err(|_| HttpRequestError::Timeout)?
    }

    async fn get_inner(
        self,
        url: reqwest::Url,
        content_type: &'static str,
        body: String,
    ) -> Result<HttpResponse, HttpRequestError> {
        let host = url.host_str().ok_or(HttpRequestError::MissingHost)?;
        let server_name = ServerName::try_from(host.to_owned())
            .map_err(|_| HttpRequestError::InvalidServerName(host.to_owned()))?;
        let stream = crate::p2p::connect(&self.config).await?;
        let tls = TlsConnector::from(self.tls)
            .connect(server_name, stream)
            .await?;
        let use_http2 = tls.get_ref().1.alpn_protocol() == Some(b"h2");
        let request = Request::builder()
            .method(http::Method::GET)
            .uri(url.as_str())
            .header(CONTENT_TYPE, content_type)
            .body(Full::new(Bytes::from(body)))?;
        let response = if use_http2 {
            let (mut sender, connection) =
                hyper::client::conn::http2::Builder::new(TokioExecutor::new())
                    .handshake(TokioIo::new(tls))
                    .await?;
            tokio::spawn(async move {
                let _ = connection.await;
            });
            sender.send_request(request).await?
        } else {
            let (mut sender, connection) =
                hyper::client::conn::http1::handshake(TokioIo::new(tls)).await?;
            tokio::spawn(async move {
                let _ = connection.await;
            });
            sender.send_request(request).await?
        };
        let status = response.status();
        let body = response.into_body().collect().await?.to_bytes();
        Ok(HttpResponse {
            status,
            body: String::from_utf8(body.to_vec())?,
        })
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MakeHttpClientError<E: std::error::Error> {
    #[error("[{n}] {0}", n = self.name())]
    SniOverride(#[from] SniOverrideError),

    #[error("[{n}] {0}", n = self.name())]
    RootCertificates(E),

    #[error("[{n}] {0}", n = self.name())]
    RootToDer(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    DerToCertificate(reqwest::Error),

    #[error("[{n}] {0}", n = self.name())]
    ToTlsClient(ToTlsClientError<E>),

    #[error("[{n}] {0}", n = self.name())]
    Build(reqwest::Error),
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum HttpRequestError {
    #[error("[{n}] {0}", n = self.name())]
    Reqwest(#[from] reqwest::Error),

    #[error("[{n}] {0}", n = self.name())]
    P2p(#[from] crate::p2p::P2pConnectError),

    #[error("[{n}] TLS handshake failed: {0}", n = self.name())]
    Tls(#[from] std::io::Error),

    #[error("[{n}] HTTP connection failed: {0}", n = self.name())]
    Hyper(#[from] hyper::Error),

    #[error("[{n}] Failed to build HTTP request: {0}", n = self.name())]
    Request(#[from] http::Error),

    #[error("[{n}] Target HTTPS URL has no host", n = self.name())]
    MissingHost,

    #[error("[{n}] Invalid target TLS server name: {0}", n = self.name())]
    InvalidServerName(String),

    #[error("[{n}] Gateway response is not UTF-8: {0}", n = self.name())]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("[{n}] P2P HTTP request timed out", n = self.name())]
    Timeout,
}
