use std::path::Path;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::thread::sleep;
use std::time::Duration;
use std::time::Instant;

use clap::Parser as _;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use tracing::error;
use tracing::info;

use crate::server::Server;
use crate::server::ServerProperties;
use crate::server::TestProperties;

mod server;
mod test_dir;
mod toml;

const TIMEOUT: Duration = Duration::from_secs(45);

// TODO: all parameter must be set with --long-param-name param-value
#[derive(clap::Parser)]
struct Args {
    server_bin: PathBuf,
    // TODO: clap should treat this parameter as optional
    server_manifest_dir: Option<PathBuf>,
    port: u16,
    set_current_endpoint: PathBuf,
}

fn main() {
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

fn run() -> Result<(), RunError> {
    let Args {
        server_bin,
        port,
        set_current_endpoint,
        server_manifest_dir,
    } = Args::parse();

    let test_dir = test_dir::test_dir()?;
    let root_ca = test_dir.path().join("root-ca");
    let test_properties = TestProperties::builder()
        .test_dir(test_dir)
        .root_ca(root_ca)
        .server_bin(server_bin)
        .server_manifest_dir(server_manifest_dir)
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
    assert!(gateway.process.borrow().id().to_string() == gateway.pid_file_contents().trim());
    let gateway_endpoint = gateway.endpoint()?;
    info!(gateway_endpoint, "gateway node is ready");
    let root_ca_cert = test_properties.root_ca.with_extension("cert");
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
    let client_cert = client.client_cert_file.with_extension("cert");
    wait_for_file(&client_cert)?;
    info!(
        client_cert = %client_cert.display(),
        "client certificate is ready; supervising mesh nodes"
    );

    loop {
        gateway.ensure_running()?;
        client.ensure_running()?;
        sleep(Duration::from_millis(250));
    }
}

fn wait_for_file(path: &Path) -> Result<(), RunError> {
    wait_until(&format!("file {}", path.display()), || {
        path.exists().then_some(())
    })
}

// TODO: report the last error
fn wait_until<T>(description: &str, mut f: impl FnMut() -> Option<T>) -> Result<T, RunError> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(value) = f() {
            return Ok(value);
        }
        if Instant::now() >= deadline {
            return Err(RunError::Timeout {
                description: description.to_owned(),
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

    #[error("[{n}] Timed out waiting for {description}", n = self.name())]
    Timeout { description: String },

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
}
