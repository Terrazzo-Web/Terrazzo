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
use tracing::error;
use tracing::info;

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
