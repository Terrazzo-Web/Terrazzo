use std::error::Error;
use std::fs;
use std::fs::OpenOptions;
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::thread::sleep;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use openssl::nid::Nid;
use openssl::x509::X509;

const CLIENT_NAME: &str = "mesh-integration-client";
const TIMEOUT: Duration = Duration::from_secs(45);

#[test]
fn mesh_client_gets_certificate_from_gateway() -> Result<(), Box<dyn Error>> {
    let server_bin = server_bin()?;
    let test_dir = test_dir()?;
    fs::create_dir_all(test_dir.join("home"))?;

    let root_ca = test_dir.join("root-ca");
    let server = ServerInstance::start(
        "gateway",
        &server_bin,
        &test_dir,
        server_config(&test_dir, "gateway", &root_ca),
        Vec::new(),
    )?;
    server.wait_until_ready()?;
    let gateway_endpoint = server.endpoint()?;
    let root_ca_cert = root_ca.with_extension("cert");
    wait_for_file(&root_ca_cert)?;

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
        Vec::new(),
    )?;
    first_client.wait_for_log("Failed to load Client Certificate")?;
    first_client.wait_for_log("Gateway returned 403 Forbidden")?;
    first_client.stop()?;

    let auth_code = server.wait_for_auth_code()?;
    assert!(
        !auth_code.is_empty(),
        "gateway logged an empty auth code; log:\n{}",
        server.log_contents()
    );

    let second_client = ServerInstance::start(
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
        vec!["--auth-code".to_owned(), auth_code],
    )?;
    wait_for_file(&client_cert.with_extension("cert"))?;
    assert_certificate_common_name(&client_cert.with_extension("cert"), CLIENT_NAME)?;
    second_client.stop()?;
    server.stop()?;
    Ok(())
}

fn server_bin() -> Result<PathBuf, Box<dyn Error>> {
    Ok(fs::canonicalize(
        std::env::args()
            .nth(1)
            .ok_or("missing server binary argument")?,
    )?)
}

fn test_dir() -> Result<PathBuf, Box<dyn Error>> {
    let base = std::env::var_os("TEST_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let dir = base.join(format!("terrazzo-mesh-integration-{unique}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn server_config(test_dir: &Path, name: &str, root_ca: &Path) -> String {
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
    child: std::cell::RefCell<Option<Child>>,
    endpoint_file: PathBuf,
    log_file: PathBuf,
}

impl ServerInstance {
    fn start(
        name: &str,
        server_bin: &Path,
        test_dir: &Path,
        config: String,
        extra_args: Vec<String>,
    ) -> Result<Self, Box<dyn Error>> {
        let config_file = test_dir.join(format!("{name}.toml"));
        let endpoint_file = test_dir.join(format!("{name}.endpoint"));
        let log_file = test_dir.join(format!("{name}.log"));
        fs::write(&config_file, config)?;

        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)?;
        let stderr = log.try_clone()?;
        let manifest_dir = server_bin
            .parent()
            .ok_or("server binary has no parent")?
            .join("cargo_root")
            .join(
                server_bin
                    .file_name()
                    .ok_or("server binary has no file name")?,
            );

        let mut command = Command::new(server_bin);
        command
            .arg("--config-file")
            .arg(&config_file)
            .arg("--set_current_endpoint")
            .arg(&endpoint_file)
            .args(extra_args)
            .env("CARGO_MANIFEST_DIR", manifest_dir)
            .env("HOME", test_dir.join("home"))
            .env("RUST_BACKTRACE", "1")
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(stderr));
        let child = command.spawn()?;

        Ok(Self {
            child: std::cell::RefCell::new(Some(child)),
            endpoint_file,
            log_file,
        })
    }

    fn wait_until_ready(&self) -> Result<(), Box<dyn Error>> {
        wait_until("server endpoint to accept TCP connections", || {
            let endpoint = self.endpoint().ok()?;
            TcpStream::connect(endpoint).ok()?;
            Some(())
        })
    }

    fn endpoint(&self) -> Result<String, Box<dyn Error>> {
        Ok(fs::read_to_string(&self.endpoint_file)?.trim().to_owned())
    }

    fn wait_for_log(&self, pattern: &str) -> Result<(), Box<dyn Error>> {
        wait_until(&format!("log containing {pattern:?}"), || {
            self.log_contents().contains(pattern).then_some(())
        })
    }

    fn wait_for_auth_code(&self) -> Result<String, Box<dyn Error>> {
        wait_until("auth code in gateway log", || {
            parse_auth_code(&self.log_contents())
        })
    }

    fn log_contents(&self) -> String {
        fs::read_to_string(&self.log_file).unwrap_or_default()
    }

    fn stop(&self) -> Result<(), Box<dyn Error>> {
        let Some(mut child) = self.child.borrow_mut().take() else {
            return Ok(());
        };
        if child.try_wait()?.is_none() {
            child.kill()?;
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

fn parse_auth_code(log: &str) -> Option<String> {
    let prefix = "Invalid auth code. Got '' expected '";
    let start = log.rfind(prefix)? + prefix.len();
    let rest = &log[start..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_owned())
}

fn wait_for_file(path: &Path) -> Result<(), Box<dyn Error>> {
    wait_until(&format!("file {}", path.display()), || {
        path.exists().then_some(())
    })
}

fn wait_until<T>(description: &str, mut f: impl FnMut() -> Option<T>) -> Result<T, Box<dyn Error>> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(value) = f() {
            return Ok(value);
        }
        if Instant::now() >= deadline {
            return Err(format!("timed out waiting for {description}").into());
        }
        sleep(Duration::from_millis(250));
    }
}

fn assert_certificate_common_name(path: &Path, expected: &str) -> Result<(), Box<dyn Error>> {
    let certificate = X509::from_pem(&fs::read(path)?)?;
    let names = certificate
        .subject_name()
        .entries_by_nid(Nid::COMMONNAME)
        .map(|entry| entry.data().as_utf8().map(|value| value.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    assert!(
        names.iter().any(|name| name.contains(expected)),
        "expected subject common name to contain {expected:?}, got {names:?}"
    );
    Ok(())
}
