use std::cell::RefCell;
use std::ffi::OsString;
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;
use std::thread::sleep;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::SystemTimeError;
use std::time::UNIX_EPOCH;

use clap::Parser as _;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use tracing::error;
use tracing::info;
use tracing::warn;

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
        .try_init()?;

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

    let test_dir = test_dir()?;
    let home_dir = test_dir.join("home");
    std::fs::create_dir_all(&home_dir).map_err(|source| RunError::CreateHomeDir {
        path: home_dir,
        source,
    })?;
    info!(test_dir = %test_dir.display(), "starting mesh harness");

    let root_ca = test_dir.join("root-ca");
    let gateway = ServerInstance::start(
        "gateway",
        &server_bin,
        &test_dir,
        server_config(&test_dir, "gateway", port, &root_ca),
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

    let client_cert = test_dir.join("client-certificate");
    let first_client = ServerInstance::start(
        "client-invalid-auth-code",
        &server_bin,
        &test_dir,
        client_config(
            &test_dir,
            "client-invalid-auth-code",
            &root_ca,
            &root_ca_cert,
            &client_cert,
            &gateway_endpoint,
        ),
        &test_dir.join("client-invalid-auth-code.endpoint"),
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
        &test_dir,
        client_config(
            &test_dir,
            "client-valid-auth-code",
            &root_ca,
            &root_ca_cert,
            &client_cert,
            &gateway_endpoint,
        ),
        &test_dir.join("client-valid-auth-code.endpoint"),
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

fn test_dir() -> Result<PathBuf, RunError> {
    let base = std::env::var_os("TEST_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let dir = base.join(format!("terrazzo-mesh-harness-{unique}"));
    std::fs::create_dir_all(&dir).map_err(|source| RunError::CreateTestDir {
        path: dir.clone(),
        source,
    })?;
    Ok(dir)
}

fn server_config(test_dir: &Path, name: &str, port: u16, root_ca: &Path) -> String {
    format!(
        r#"
[server]
host = "127.0.0.1"
port = {port}
pidfile = "{pidfile}"
private_root_ca = "{root_ca}"
token_lifetime = "5m"
token_refresh = "4m 50s"
config_file_watcher = false
certificate_renewal_threshold = "30days"
"#,
        pidfile = toml_path(&test_dir.join(format!("{name}.pid"))),
        root_ca = toml_path(root_ca),
    )
}

fn client_config(
    test_dir: &Path,
    name: &str,
    root_ca: &Path,
    root_ca_cert: &Path,
    client_cert: &Path,
    gateway_endpoint: &str,
) -> String {
    format!(
        r#"
[server]
host = "127.0.0.1"
port = 0
pidfile = "{pidfile}"
private_root_ca = "{root_ca}"
token_lifetime = "5m"
token_refresh = "4m 50s"
config_file_watcher = false
certificate_renewal_threshold = "30days"

[mesh]
client_name = "{client_name}"
gateway_url = "https://{gateway_endpoint}"
gateway_pki = "{root_ca_cert}"
client_certificate = "{client_cert}"
client_certificate_renewal = "30days"

[mesh.retry_strategy]
fixed = "1s"
"#,
        pidfile = toml_path(&test_dir.join(format!("{name}.pid"))),
        root_ca = toml_path(root_ca),
        client_name = CLIENT_NAME,
        gateway_endpoint = gateway_endpoint,
        root_ca_cert = toml_path(root_ca_cert),
        client_cert = toml_path(client_cert),
    )
}

fn toml_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

struct ServerInstance {
    name: String,
    child: RefCell<Option<Child>>,
    endpoint_file: PathBuf,
    log_file: PathBuf,
}

impl ServerInstance {
    fn start(
        name: &str,
        server_bin: &Path,
        test_dir: &Path,
        config: String,
        endpoint_file: &Path,
        server_args: &[OsString],
        extra_args: Vec<OsString>,
    ) -> Result<Self, RunError> {
        let config_file = test_dir.join(format!("{name}.toml"));
        let log_file = test_dir.join(format!("{name}.log"));
        std::fs::write(&config_file, config).map_err(|source| RunError::WriteConfig {
            name: name.to_owned(),
            path: config_file.clone(),
            source,
        })?;

        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .map_err(|source| RunError::OpenLog {
                name: name.to_owned(),
                path: log_file.clone(),
                source,
            })?;
        let stderr = log.try_clone().map_err(|source| RunError::CloneLog {
            name: name.to_owned(),
            path: log_file.clone(),
            source,
        })?;

        let mut command = Command::new(server_bin);
        info!(
            name,
            config_file = %config_file.display(),
            endpoint_file = %endpoint_file.display(),
            log_file = %log_file.display(),
            "starting server node"
        );
        command
            .arg("--config-file")
            .arg(&config_file)
            .arg("--set_current_endpoint")
            .arg(endpoint_file)
            .args(server_args)
            .args(extra_args)
            .env("CARGO_MANIFEST_DIR", server_manifest_dir(server_bin)?)
            .env("HOME", test_dir.join("home"))
            .env("RUST_BACKTRACE", "1")
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(stderr));
        let child = command.spawn().map_err(|source| RunError::SpawnServer {
            name: name.to_owned(),
            server_bin: server_bin.to_owned(),
            source,
        })?;
        info!(name, pid = child.id(), "server node started");

        Ok(Self {
            name: name.to_owned(),
            child: RefCell::new(Some(child)),
            endpoint_file: endpoint_file.to_owned(),
            log_file,
        })
    }

    fn wait_until_ready(&self) -> Result<(), RunError> {
        wait_until("server endpoint to accept TCP connections", || {
            let endpoint = self.endpoint().ok()?;
            TcpStream::connect(endpoint).ok()?;
            Some(())
        })
    }

    fn endpoint(&self) -> Result<String, RunError> {
        Ok(std::fs::read_to_string(&self.endpoint_file)
            .map_err(|source| RunError::ReadEndpoint {
                path: self.endpoint_file.clone(),
                source,
            })?
            .trim()
            .to_owned())
    }

    fn wait_for_log(&self, pattern: &str) -> Result<(), RunError> {
        wait_until(&format!("log containing {pattern:?}"), || {
            self.log_contents().contains(pattern).then_some(())
        })
    }

    fn wait_for_auth_code(&self) -> Result<String, RunError> {
        wait_until("auth code in gateway log", || {
            parse_auth_code(&self.log_contents())
        })
    }

    fn log_contents(&self) -> String {
        std::fs::read_to_string(&self.log_file).unwrap_or_default()
    }

    fn ensure_running(&self) -> Result<(), RunError> {
        let mut child = self.child.borrow_mut();
        let child = child.as_mut().ok_or_else(|| RunError::NodeNotRunning {
            name: self.name.clone(),
        })?;
        if let Some(status) = child.try_wait().map_err(|source| RunError::TryWait {
            name: self.name.clone(),
            source,
        })? {
            return Err(RunError::NodeExited {
                name: self.name.clone(),
                status,
                log: self.log_contents(),
            });
        }
        Ok(())
    }

    fn stop(&self) -> Result<(), RunError> {
        let Some(mut child) = self.child.borrow_mut().take() else {
            return Ok(());
        };
        if child
            .try_wait()
            .map_err(|source| RunError::TryWait {
                name: self.name.clone(),
                source,
            })?
            .is_none()
        {
            warn!(name = %self.name, pid = child.id(), "stopping server node");
            child.kill().map_err(|source| RunError::KillServer {
                name: self.name.clone(),
                source,
            })?;
        }
        let _ = child.wait();
        Ok(())
    }
}

impl Drop for ServerInstance {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn server_manifest_dir(server_bin: &Path) -> Result<PathBuf, RunError> {
    Ok(server_bin
        .parent()
        .ok_or_else(|| RunError::ServerBinMissingParent {
            server_bin: server_bin.to_owned(),
        })?
        .join("cargo_root")
        .join(
            server_bin
                .file_name()
                .ok_or_else(|| RunError::ServerBinMissingFileName {
                    server_bin: server_bin.to_owned(),
                })?,
        ))
}

fn parse_auth_code(log: &str) -> Option<String> {
    let prefix = "Invalid auth code. Got '' expected '";
    let start = log.rfind(prefix)? + prefix.len();
    let rest = &log[start..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_owned())
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
    #[error("[{n}] {0}", n = self.name())]
    SystemTime(#[from] SystemTimeError),

    #[error("[{n}] Failed to create test directory {path:?}: {source}", n = self.name())]
    CreateTestDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("[{n}] Failed to create home directory {path:?}: {source}", n = self.name())]
    CreateHomeDir {
        path: PathBuf,
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
