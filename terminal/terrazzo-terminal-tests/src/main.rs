use std::ffi::OsString;
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

use crate::config::client_config;
use crate::config::server_config;
use crate::server::ServerInstance;

mod config;
mod server;
mod test_dir;

const TIMEOUT: Duration = Duration::from_secs(45);

#[derive(clap::Parser)]
struct Args {
    server_bin: PathBuf,
    port: u16,
    set_current_endpoint: PathBuf,
    #[arg(last = true)]
    server_args: Vec<OsString>,
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
        server_args,
    } = Args::parse();

    let test_dir = test_dir::test_dir()?;
    let root_ca = test_dir.path().join("root-ca");
    let gateway = ServerInstance::start(
        "gateway",
        &server_bin,
        test_dir.path(),
        server_config(test_dir.path(), "gateway", port, &root_ca),
        &set_current_endpoint,
        &server_args,
        Vec::new(),
    )?;
    gateway.wait_until_ready()?;
    let gateway_endpoint = gateway.endpoint()?;
    info!(gateway_endpoint, "gateway node is ready");
    let root_ca_cert = root_ca.with_extension("cert");
    wait_for_file(&root_ca_cert)?;
    info!(root_ca_cert = %root_ca_cert.display(), "gateway root certificate is ready");

    let client_cert = test_dir.path().join("client-certificate");
    let first_client = ServerInstance::start(
        "client-invalid-auth-code",
        &server_bin,
        test_dir.path(),
        client_config(
            test_dir.path(),
            "client-invalid-auth-code",
            &root_ca,
            &root_ca_cert,
            &client_cert,
            &gateway_endpoint,
        ),
        &test_dir.path().join("client-invalid-auth-code.endpoint"),
        &server_args,
        Vec::new(),
    )?;
    first_client.wait_for_log("Failed to load Client Certificate")?;
    first_client.wait_for_log("Gateway returned 403 Forbidden")?;
    first_client.stop()?;
    info!("invalid-auth client stopped after expected gateway rejection");

    let auth_code = gateway.wait_for_auth_code()?;
    if auth_code.is_empty() {
        return Err(RunError::EmptyAuthCode {
            log: gateway.log_contents(),
        });
    }
    info!("gateway auth code was discovered");

    let client = ServerInstance::start(
        "client-valid-auth-code",
        &server_bin,
        test_dir.path(),
        client_config(
            test_dir.path(),
            "client-valid-auth-code",
            &root_ca,
            &root_ca_cert,
            &client_cert,
            &gateway_endpoint,
        ),
        &test_dir.path().join("client-valid-auth-code.endpoint"),
        &server_args,
        vec!["--auth-code".into(), auth_code.into()],
    )?;
    wait_for_file(&client_cert.with_extension("cert"))?;
    info!(
        client_cert = %client_cert.with_extension("cert").display(),
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

    #[error("[{n}] Failed to write config for {name:?} to {path:?}: {source}", n = self.name())]
    WriteConfig {
        name: String,
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("[{n}] Failed to open log for {name:?} at {path:?}: {source}", n = self.name())]
    OpenLog {
        name: String,
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("[{n}] Failed to clone log for {name:?} at {path:?}: {source}", n = self.name())]
    CloneLog {
        name: String,
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

    #[error("[{n}] Server binary has no parent: {server_bin:?}", n = self.name())]
    ServerBinMissingParent { server_bin: PathBuf },

    #[error("[{n}] Server binary has no file name: {server_bin:?}", n = self.name())]
    ServerBinMissingFileName { server_bin: PathBuf },

    #[error("[{n}] Timed out waiting for {description}", n = self.name())]
    Timeout { description: String },

    #[error("[{n}] Gateway logged an empty auth code; log:\n{log}", n = self.name())]
    EmptyAuthCode { log: String },

    #[error("[{n}] {name} is not running", n = self.name())]
    NodeNotRunning { name: String },

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
