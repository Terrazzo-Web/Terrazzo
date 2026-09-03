use std::path::Path;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::task::Poll;
use std::thread::sleep;
use std::time::Duration;
use std::time::Instant;

use clap::Parser as _;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::nid::Nid;
use openssl::x509::X509;
use tokio::sync::mpsc;
use tracing::error;
use tracing::info;
use trz_gateway_common::p2p::GOOGLE_STUN;
use trz_gateway_common::p2p::peer_connection::IceServer;
use trz_gateway_common::p2p::peer_connection::LocalIceEvent;
use trz_gateway_common::p2p::peer_connection::PeerConnectionBuilder;

use crate::server::Server;
use crate::server::ServerProperties;
use crate::server::TestProperties;
use crate::signal_handler::install_signal_handlers;
use crate::signal_handler::termination_requested;

mod server;
mod signal_handler;
mod test_dir;
mod toml;

const TIMEOUT: Duration = Duration::from_secs(45);

#[derive(clap::Parser)]
struct Args {
    #[arg(long)]
    server_bin: PathBuf,

    #[arg(long, default_value_t = 0)]
    port: u16,

    #[arg(long)]
    set_current_endpoint: PathBuf,
}

pub fn main() {
    let () = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .try_init()
        .unwrap();

    match run() {
        Ok(()) => {}
        Err(error) => {
            error!("{error}");
            std::process::exit(1);
        }
    }
}

pub fn p2p_main() {
    let () = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .try_init()
        .unwrap();

    match p2p_run() {
        Ok(()) => {}
        Err(error) => {
            error!(%error);
            std::process::exit(1);
        }
    }
}

fn p2p_run() -> Result<(), Box<dyn std::error::Error>> {
    install_signal_handlers()?;
    let Args {
        server_bin,
        port,
        set_current_endpoint,
    } = Args::parse();

    tokio::runtime::Runtime::new()?.block_on(assert_google_stun_candidate())?;
    info!(
        GOOGLE_STUN,
        "Google STUN returned a server-reflexive candidate"
    );

    let test_dir = test_dir::test_dir()?;
    let signaling_test_properties = TestProperties::builder()
        .test_dir(test_dir.clone())
        .root_ca(test_dir.join("signaling-root-ca"))
        .server_bin(server_bin.clone())
        .build()
        .into();
    let target_test_properties = TestProperties::builder()
        .test_dir(test_dir.clone())
        .root_ca(test_dir.join("target-root-ca"))
        .server_bin(server_bin.clone())
        .build()
        .into();
    let client_test_properties = TestProperties::builder()
        .test_dir(test_dir.clone())
        .root_ca(test_dir.join("client-root-ca"))
        .server_bin(server_bin)
        .build()
        .into();

    let signaling = Server::start(
        ServerProperties::builder()
            .test_properties(&signaling_test_properties)
            .name("signaling")
            .mode(server::Mode::Gateway)
            .port(0)
            .build(),
        &[],
    )?;
    signaling.wait_until_ready()?;
    let signaling_endpoint = signaling.endpoint()?;
    let signaling_root_cert = signaling_test_properties
        .root_ca
        .with_added_extension("cert");
    wait_for_file(&signaling_root_cert)?;
    info!(signaling_endpoint, "signaling node is ready");

    let server_name = format!("terminal-p2p-{}", std::process::id());
    let gateway = Server::start(
        ServerProperties::builder()
            .test_properties(&target_test_properties)
            .name("gateway-p2p")
            .mode(server::Mode::GatewayP2p {
                signaling_endpoint: signaling_endpoint.clone(),
                server_name: server_name.clone(),
            })
            .port(port)
            .build(),
        &[],
    )?;
    gateway.wait_until_ready()?;
    gateway.wait_for_log("Registered outbound signaling WebSocket")?;
    let gateway_endpoint = gateway.endpoint()?;
    let target_root_cert = target_test_properties.root_ca.with_added_extension("cert");
    wait_for_file(&target_root_cert)?;
    info!(gateway_endpoint, "P2P gateway node is registered");

    let invalid_client = Server::start(
        ServerProperties::builder()
            .test_properties(&client_test_properties)
            .name("client-p2p-invalid-auth-code")
            .mode(server::Mode::ClientP2p {
                signaling_endpoint: signaling_endpoint.clone(),
                server_name: server_name.clone(),
                gateway_pki: target_root_cert.clone(),
            })
            .build(),
        &[],
    )?;
    invalid_client.wait_for_log("Failed to load Client Certificate")?;
    invalid_client.wait_for_log("Gateway returned 403 Forbidden")?;
    invalid_client.stop()?;

    let auth_code = gateway.wait_for_auth_code()?;
    let client = Server::start(
        ServerProperties::builder()
            .test_properties(&client_test_properties)
            .name("client-p2p")
            .mode(server::Mode::ClientP2p {
                signaling_endpoint,
                server_name,
                gateway_pki: target_root_cert.clone(),
            })
            .build(),
        &["--auth-code".into(), auth_code.into()],
    )?;
    let client_cert = client.client_cert_file.with_added_extension("cert");
    wait_for_file(&client_cert)?;
    if let Err(error) = client.wait_for_log_line(&["Serving", "client_name", "test-client"]) {
        error!(
            log = client.log_contents(),
            "P2P client tunnel did not become ready"
        );
        error!(log = gateway.log_contents(), "P2P gateway log");
        return Err(error.into());
    }
    verify_client_certificate(&client_cert, &target_root_cert, &signaling_root_cert)?;

    // Publish the browser endpoint only after the WebRTC client is ready. The
    // Playwright wrapper cannot start the test early and race mesh registration.
    std::fs::write(&set_current_endpoint, &gateway_endpoint)?;
    info!(gateway_endpoint, "P2P mesh is ready for Playwright");

    loop {
        if termination_requested() {
            client.stop()?;
            gateway.stop()?;
            signaling.stop()?;
            remove_test_dir(&test_dir)?;
            return Ok(());
        }
        signaling.ensure_running()?;
        gateway.ensure_running()?;
        client.ensure_running()?;
        sleep(Duration::from_millis(250));
    }
}

fn verify_client_certificate(
    client_certificate: &Path,
    target_root_certificate: &Path,
    signaling_root_certificate: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let certificate = X509::from_pem(&std::fs::read(client_certificate)?)?;
    let root = X509::from_pem(&std::fs::read(target_root_certificate)?)?;
    let root_public_key = root.public_key()?;
    if !certificate.verify(&root_public_key)? {
        return Err("client certificate is not signed by the target gateway root".into());
    }
    let signaling_root = X509::from_pem(&std::fs::read(signaling_root_certificate)?)?;
    if certificate.verify(&signaling_root.public_key()?)? {
        return Err("client certificate was unexpectedly signed by the signaling root".into());
    }
    let common_name = certificate
        .subject_name()
        .entries_by_nid(Nid::COMMONNAME)
        .next()
        .ok_or("client certificate has no common name")?
        .data()
        .to_string()?
        .to_string();
    if common_name != "test-client" {
        return Err(format!("unexpected client certificate common name: {common_name}").into());
    }
    Ok(())
}

async fn assert_google_stun_candidate() -> Result<(), Box<dyn std::error::Error>> {
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
    let candidate = tokio::time::timeout(Duration::from_secs(20), async {
        while let Some(event) = local_ice_rx.recv().await {
            match event {
                LocalIceEvent::Candidate(candidate)
                    if candidate.candidate.contains(" typ srflx") =>
                {
                    return Ok::<(), Box<dyn std::error::Error>>(());
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

fn run() -> Result<(), RunError> {
    install_signal_handlers()?;

    let Args {
        server_bin,
        port,
        set_current_endpoint,
    } = Args::parse();

    let test_dir = test_dir::test_dir()?;
    let root_ca = test_dir.join("root-ca");
    let test_properties = TestProperties::builder()
        .test_dir(test_dir.clone())
        .root_ca(root_ca)
        .server_bin(server_bin)
        .build()
        .into();

    let gateway_properties = ServerProperties::builder()
        .test_properties(&test_properties)
        .name("gateway")
        .mode(server::Mode::Gateway)
        .port(port)
        .set_current_endpoint(set_current_endpoint)
        .build();
    let gateway = Server::start(gateway_properties, &[])?;
    gateway.wait_until_ready()?;
    let gateway_endpoint = gateway.endpoint()?;
    info!(gateway_endpoint, "gateway node is ready");
    let root_ca_cert = test_properties.root_ca.with_added_extension("cert");
    wait_for_file(&root_ca_cert)?;
    info!(root_ca_cert = %root_ca_cert.display(), "gateway root certificate is ready");

    let client_properties = ServerProperties::builder()
        .test_properties(&test_properties)
        .name("client-invalid-auth-code")
        .mode(server::Mode::Client {
            gateway_endpoint: gateway_endpoint.clone(),
        })
        .build();
    let first_client = Server::start(client_properties, &[])?;
    first_client.wait_for_log("Failed to load Client Certificate")?;
    first_client.wait_for_log("Gateway returned 403 Forbidden")?;
    first_client.stop()?;
    info!("invalid-auth client stopped after expected gateway rejection");

    let auth_code = gateway.wait_for_auth_code()?;
    info!("gateway auth code was discovered");

    let client_properties = ServerProperties::builder()
        .test_properties(&test_properties)
        .name("client")
        .mode(server::Mode::Client { gateway_endpoint })
        .build();
    let client = Server::start(client_properties, &["--auth-code".into(), auth_code.into()])?;
    let client_cert = client.client_cert_file.with_added_extension("cert");
    wait_for_file(&client_cert)?;
    info!(
        client_cert = %client_cert.display(),
        "client certificate is ready; supervising mesh nodes"
    );

    loop {
        if termination_requested() {
            info!("termination requested; stopping mesh nodes");
            let client_stop_result = client.stop();
            let gateway_stop_result = gateway.stop();
            remove_test_dir(&test_dir)?;
            client_stop_result?;
            gateway_stop_result?;
            return Ok(());
        }
        gateway.ensure_running()?;
        client.ensure_running()?;
        sleep(Duration::from_millis(250));
    }
}

fn remove_test_dir(test_dir: &Path) -> Result<(), RunError> {
    std::fs::remove_dir_all(test_dir).or_else(|source| match source.kind() {
        std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(RunError::RemoveTestDir {
            path: test_dir.to_path_buf(),
            source,
        }),
    })
}

fn wait_for_file(path: &Path) -> Result<(), RunError> {
    wait_until(&format!("file {}", path.display()), || {
        match path.exists() {
            true => Poll::Ready(Ok(())),
            false => Poll::Pending,
        }
    })
}

fn wait_until<T>(
    description: &str,
    mut f: impl FnMut() -> Poll<Result<T, RunError>>,
) -> Result<T, RunError> {
    let deadline = Instant::now() + TIMEOUT;
    let mut last_error = None;
    loop {
        if termination_requested() {
            return Err(RunError::Terminated);
        }
        match f() {
            Poll::Ready(Ok(value)) => return Ok(value),
            Poll::Ready(Err(error)) => last_error = Some(Box::new(error)),
            Poll::Pending => {}
        }
        if Instant::now() >= deadline {
            return Err(RunError::Timeout {
                description: description.to_owned(),
                last_error,
            });
        }
        sleep(Duration::from_millis(250));
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
enum RunError {
    #[error("[{n}] Failed to create test directory under {base:?}: {source}", n = self.name())]
    CreateTestDir {
        base: Option<PathBuf>,
        source: std::io::Error,
    },

    #[error("[{n}] Failed to remove test directory at {path:?}: {source}", n = self.name())]
    RemoveTestDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("[{n}] Failed to write config for {server:?} to {path:?}: {source}", n = self.name())]
    WriteConfig {
        server: String,
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("[{n}] Failed to open log for {name:?} at {path:?}: {source}", n = self.name())]
    OpenLog {
        name: String,
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("[{n}] Failed to clone log for {server:?} at {path:?}: {source}", n = self.name())]
    CloneLog {
        server: String,
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("[{n}] Failed to start {name:?} from {server_bin:?}: {source}", n = self.name())]
    SpawnServer {
        name: String,
        server_bin: PathBuf,
        source: std::io::Error,
    },

    #[error("[{n}] Failed to read endpoint file {path:?}: {source}", n = self.name())]
    ReadEndpoint {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("[{n}] Failed to connect to {endpoint}: {source}", n = self.name())]
    Connect {
        endpoint: String,
        source: std::io::Error,
    },

    #[error(
        "[{n}] Timed out waiting for {description}{last_error}",
        n = self.name(),
        last_error = .last_error.as_ref().map(|error| format!("; last error: {error}")).unwrap_or_default(),
    )]
    Timeout {
        description: String,
        last_error: Option<Box<RunError>>,
    },

    #[error("[{n}] Gateway logged an empty auth code; log:\n{log}", n = self.name())]
    EmptyAuthCode { log: String },

    #[error("[{n}] Failed to poll {name}: {source}", n = self.name())]
    TryWait {
        name: String,
        source: std::io::Error,
    },

    #[error("[{n}] {name} exited with {status}; log:\n{log}", n = self.name())]
    NodeExited {
        name: String,
        status: ExitStatus,
        log: String,
    },

    #[error("[{n}] Failed to stop {name}: {source}", n = self.name())]
    KillServer {
        name: String,
        source: std::io::Error,
    },

    #[error("[{n}] Failed to create temp dir for {name}: {source}", n = self.name())]
    ServerTempDir {
        name: String,
        source: std::io::Error,
    },

    #[error("[{n}] Failed to install handler for signal {signal}: {source}", n = self.name())]
    InstallSignalHandler {
        signal: libc::c_int,
        source: std::io::Error,
    },

    #[error("[{n}] Terminated", n = self.name())]
    Terminated,
}
