use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use futures::SinkExt as _;
use futures::StreamExt as _;
use openssl::nid::Nid;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use trz_gateway_common::id::ClientName;
use trz_gateway_common::p2p::peer_connection::IceServer;
use trz_gateway_common::p2p::peer_connection::LocalIceEvent;
use trz_gateway_common::p2p::peer_connection::PeerConnectionBuilder;
use trz_gateway_common::p2p::protocol::PROTOCOL_VERSION;
use trz_gateway_common::p2p::protocol::SignalMessage;
use trz_gateway_common::security_configuration::certificate::CertificateConfig as _;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificate;
use trz_gateway_server::auth_code::AuthCode as ServerAuthCode;
use trz_gateway_server::server::Server;
use trz_gateway_server::server::gateway_config::GatewayConfig as _;
use trz_gateway_server::server::gateway_config::p2p::P2pRegistrationConfig;
use uuid::Uuid;

use super::test_gateway_config::TestGatewayConfig;
use super::test_gateway_config::use_temp_dir;
use crate::client::config::ClientConfig;
use crate::client::config::ClientTransport;
use crate::client::config::P2pClientConfig;
use crate::load_client_certificate::make_client_certificate;

const GOOGLE_STUN: &str = "stun:stun.l.google.com:19302";
const PHASE_TIMEOUT: Duration = Duration::from_secs(20);
const OVERALL_TIMEOUT: Duration = Duration::from_secs(75);

/// Network test: run explicitly with
/// `cargo test -p trz-gateway-client p2p_certificate_via_google_stun -- --ignored`.
#[tokio::test]
#[ignore = "requires outbound DNS and UDP access to Google's public STUN service"]
async fn p2p_certificate_via_google_stun() -> Result<(), Box<dyn Error>> {
    let _temp_dir = use_temp_dir();
    assert_google_stun_candidate().await?;

    let signaling_config = TestGatewayConfig::new();
    let (_signaling, signaling_handle, _signaling_crash) =
        Server::run(signaling_config.clone()).await?;
    let signaling_url = format!("http://localhost:{}", signaling_config.port());
    let server_name = ClientName::from(format!("p2p-test-{}", Uuid::new_v4()));
    let registration = P2pRegistrationConfig::new(signaling_url.clone(), server_name.clone());
    let target_config = TestGatewayConfig::new_with_p2p(registration);
    let target = Server::run(target_config.clone()).await;
    let (_target, target_handle, _target_crash) = match target {
        Ok(target) => target,
        Err(error) => {
            let _ = signaling_handle.stop("P2P target setup failed").await;
            return Err(Box::<dyn Error>::from(error));
        }
    };

    let result = tokio::time::timeout(OVERALL_TIMEOUT, async {
        wait_until_registered(&signaling_url, &server_name).await?;

        // This configuration intentionally contains no target socket address or port.
        let client_name = ClientName::from(format!("p2p-client-{}", Uuid::new_v4()));
        let target_root = target_config.root_ca();
        let config = P2pCertificateConfig::new(
            target_root.clone(),
            client_name.clone(),
            "localhost",
            p2p_config(&signaling_url, server_name.clone()),
        );
        let certificate =
            make_client_certificate(&config, ServerAuthCode::current().to_string().into()).await?;

        let root = target_root.certificate()?;
        let root_public_key = root.certificate.public_key()?;
        assert!(certificate.certificate.verify(&root_public_key)?);
        let common_name = certificate
            .certificate
            .subject_name()
            .entries_by_nid(Nid::COMMONNAME)
            .next()
            .ok_or("issued certificate has no common name")?
            .data()
            .to_string()?;
        assert_eq!(client_name.as_ref(), common_name);

        // The signaling gateway has a different root. A certificate signed by the
        // target root proves that HTTP application data terminated at the target.
        let signaling_root = signaling_config.root_ca().certificate()?;
        let signaling_public_key = signaling_root.certificate.public_key()?;
        assert!(!certificate.certificate.verify(&signaling_public_key)?);

        let unknown = P2pCertificateConfig::new(
            target_root.clone(),
            "unknown-server-client".into(),
            "localhost",
            p2p_config(&signaling_url, format!("unknown-{}", Uuid::new_v4()).into()),
        );
        let error = make_client_certificate(&unknown, ServerAuthCode::current().to_string().into())
            .await
            .expect_err("an unknown P2P server must not be reachable");
        assert!(error.to_string().to_lowercase().contains("timed out"));

        let wrong_tls_name = P2pCertificateConfig::new(
            target_root,
            "wrong-tls-name-client".into(),
            "not-localhost.invalid",
            p2p_config(&signaling_url, server_name),
        );
        let error = make_client_certificate(
            &wrong_tls_name,
            ServerAuthCode::current().to_string().into(),
        )
        .await
        .expect_err("WebRTC must not bypass target TLS hostname verification");
        assert!(error.to_string().contains("TLS handshake failed"));

        Ok::<(), Box<dyn Error>>(())
    })
    .await;

    let target_stop = target_handle.stop("End of P2P certificate test").await;
    let signaling_stop = signaling_handle.stop("End of P2P certificate test").await;
    result??;
    target_stop?;
    signaling_stop?;
    Ok(())
}

fn p2p_config(signaling_url: &str, server_name: ClientName) -> P2pClientConfig {
    let mut config = P2pClientConfig::new(signaling_url, server_name);
    config.signaling_timeout = Duration::from_secs(5);
    config.handshake_timeout = PHASE_TIMEOUT;
    config.connect_timeout = PHASE_TIMEOUT;
    config
}

#[derive(Debug)]
struct P2pCertificateConfig {
    root: Arc<PemCertificate>,
    client_name: ClientName,
    tls_name: String,
    transport: ClientTransport,
}

impl P2pCertificateConfig {
    fn new(
        root: Arc<PemCertificate>,
        client_name: ClientName,
        tls_name: impl Into<String>,
        p2p: P2pClientConfig,
    ) -> Self {
        Self {
            root,
            client_name,
            tls_name: tls_name.into(),
            transport: ClientTransport::WebRtc(p2p),
        }
    }
}

impl ClientConfig for P2pCertificateConfig {
    fn base_url(&self) -> impl std::fmt::Display {
        format!("https://{}", self.tls_name)
    }

    fn client_name(&self) -> ClientName {
        self.client_name.clone()
    }

    type GatewayPki = Arc<PemCertificate>;

    fn gateway_pki(&self) -> Self::GatewayPki {
        self.root.clone()
    }

    fn transport(&self) -> ClientTransport {
        self.transport.clone()
    }
}

async fn wait_until_registered(
    signaling_url: &str,
    server_name: &ClientName,
) -> Result<(), Box<dyn Error>> {
    let url = format!(
        "{}/p2p/connect/{}",
        signaling_url.replace("http://", "ws://"),
        server_name.as_ref()
    );
    tokio::time::timeout(PHASE_TIMEOUT, async {
        loop {
            match connect_async(&url).await {
                Ok((mut socket, _)) => {
                    send_signal(
                        &mut socket,
                        &SignalMessage::Hello {
                            protocol_version: PROTOCOL_VERSION,
                        },
                    )
                    .await?;
                    let connection_id = loop {
                        match socket.next().await.ok_or("signaling socket closed")?? {
                            Message::Text(text) => {
                                let message: SignalMessage = serde_json::from_str(&text)?;
                                if let SignalMessage::Start { connection_id } = message {
                                    break connection_id;
                                }
                            }
                            Message::Ping(_) | Message::Pong(_) => continue,
                            _ => return Err("unexpected signaling message".into()),
                        }
                    };
                    send_signal(&mut socket, &SignalMessage::Cancel { connection_id }).await?;
                    let _ = socket.close(None).await;
                    return Ok::<(), Box<dyn Error>>(());
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    })
    .await??;
    Ok(())
}

async fn send_signal<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    message: &SignalMessage,
) -> Result<(), Box<dyn Error>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    socket
        .send(Message::Text(serde_json::to_string(message)?.into()))
        .await?;
    Ok(())
}

async fn assert_google_stun_candidate() -> Result<(), Box<dyn Error>> {
    let (local_ice_tx, mut local_ice_rx) = mpsc::channel(64);
    let peer = PeerConnectionBuilder::new(
        vec![IceServer {
            urls: vec![GOOGLE_STUN.to_owned()],
            ..IceServer::default()
        }],
        local_ice_tx,
    )
    .build()
    .await?;
    let _data_channel = peer.create_reliable_data_channel().await?;
    let _offer = peer.create_offer().await?;
    let candidate = tokio::time::timeout(PHASE_TIMEOUT, async {
        while let Some(event) = local_ice_rx.recv().await {
            match event {
                // Google is the only configured discovery server, so any
                // server-reflexive candidate in this peer came from that service.
                LocalIceEvent::Candidate(candidate)
                    if candidate.candidate.contains(" typ srflx") =>
                {
                    return Ok::<_, Box<dyn Error>>(candidate);
                }
                LocalIceEvent::Error(error) => return Err(error.into()),
                LocalIceEvent::EndOfCandidates => break,
                LocalIceEvent::Candidate(_) => {}
            }
        }
        Err("Google STUN returned no server-reflexive ICE candidate".into())
    })
    .await;
    let close = peer.close().await;
    candidate??;
    close?;
    Ok(())
}
