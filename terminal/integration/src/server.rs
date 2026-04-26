use std::borrow::Cow;
use std::cell::RefCell;
use std::ffi::OsString;
use std::net::TcpStream;
use std::ops::Deref;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::rc::Rc;

use tempfile::TempDir;
use tracing::info;
use tracing::info_span;
use tracing::warn;
use typed_builder::TypedBuilder;

use crate::RunError;
use crate::toml::client_toml;
use crate::toml::server_toml;
use crate::wait_until;

#[derive(TypedBuilder)]
pub struct TestProperties {
    /// A temp folder allocated to run the test harness
    test_dir: TempDir,

    /// Suffix added to generate the path where the server reports the dynamic port number
    #[builder(default = "current_endpoint".into(), setter(into))]
    set_current_endpoint_file_suffix: Cow<'static, str>,

    /// Suffix added to generate server logs paths
    #[builder(default = "logs".into(), setter(into))]
    logs_path_suffix: Cow<'static, str>,

    /// Suffix added to generate server config.toml file paths
    #[builder(default = "config.toml".into(), setter(into))]
    config_file_suffix: Cow<'static, str>,

    /// Suffix added to generate pid file paths
    #[builder(default = "pid".into(), setter(into))]
    pid_file_suffix: Cow<'static, str>,

    /// Suffix added to generate client certificate file paths
    #[builder(default = "client-cert".into(), setter(into))]
    client_cert_file_suffix: Cow<'static, str>,

    /// Path to the root CA
    pub root_ca: PathBuf,

    /// Path to the server executable
    server_bin: PathBuf,

    /// Path to the server crate manifest dir
    #[builder(default)]
    server_manifest_dir: Option<PathBuf>,
}

impl TestProperties {
    fn get_test_temp_path(&self, suffix: impl AsRef<str>) -> PathBuf {
        self.test_dir.path().join(suffix.as_ref())
    }
}

#[derive(TypedBuilder)]
pub struct ServerProperties {
    #[builder(setter(transform = |p: &Rc<TestProperties>| p.clone()))]
    test_properties: Rc<TestProperties>,

    mode: Mode,

    #[builder(setter(into))]
    name: String,

    #[builder(default = 0)]
    port: u16,

    #[builder(default, setter(strip_option))]
    set_current_endpoint: Option<PathBuf>,
}

pub enum Mode {
    Gateway,
    Client { gateway_endpoint: String },
}

impl Deref for ServerProperties {
    type Target = TestProperties;

    fn deref(&self) -> &Self::Target {
        &self.test_properties
    }
}

impl ServerProperties {
    fn get_temp_path(&self, suffix: impl AsRef<str>) -> PathBuf {
        self.get_test_temp_path(&self.name).join(suffix.as_ref())
    }
}

pub struct Server {
    server_properties: ServerProperties,
    pub client_cert_file: PathBuf,
    pub log_file: PathBuf,
    pub pid_file: PathBuf,
    pub endpoint_file: PathBuf,
    pub process: RefCell<Child>,
}

impl Deref for Server {
    type Target = ServerProperties;

    fn deref(&self) -> &Self::Target {
        &self.server_properties
    }
}

impl Server {
    pub fn start(properties: ServerProperties, server_args: &[OsString]) -> Result<Self, RunError> {
        let name = &properties.name;
        let _span = info_span!("Start", server = name).entered();
        let config_file = properties.get_temp_path(&properties.config_file_suffix);
        let log_file = properties.get_temp_path(&properties.logs_path_suffix);
        let pid_file = properties.get_temp_path(&properties.pid_file_suffix);
        let client_cert_file = properties.get_temp_path(&properties.client_cert_file_suffix);
        let endpoint_file = if let Some(set_current_endpoint) = &properties.set_current_endpoint {
            set_current_endpoint.clone()
        } else {
            properties.get_test_temp_path(&properties.set_current_endpoint_file_suffix)
        };
        let config = match &properties.mode {
            Mode::Gateway => server_toml(&pid_file, properties.port, &properties.root_ca),
            Mode::Client { gateway_endpoint } => client_toml(
                &pid_file,
                &properties.root_ca,
                &properties.root_ca.with_added_extension("cert"),
                &client_cert_file,
                gateway_endpoint,
            ),
        };
        std::fs::write(&config_file, config).map_err(|source| RunError::WriteConfig {
            server: name.clone(),
            path: config_file.clone(),
            source,
        })?;

        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .map_err(|source| RunError::OpenLog {
                name: name.clone(),
                path: log_file.clone(),
                source,
            })?;
        let stderr = log.try_clone().map_err(|source| RunError::CloneLog {
            server: name.clone(),
            path: log_file.clone(),
            source,
        })?;

        let mut command = Command::new(&properties.server_bin);
        info!(
            config_file = %config_file.display(),
            log_file = %log_file.display(),
            "Starting server node"
        );
        command.arg("--config-file");
        command.arg(&config_file);
        command.arg("--set_current_endpoint");
        command.arg(&endpoint_file);
        command.args(server_args);
        if let Some(server_manifest_dir) = &properties.server_manifest_dir {
            command.env("CARGO_MANIFEST_DIR", server_manifest_dir);
        }
        command.env("RUST_BACKTRACE", "1");
        command.stdout(Stdio::from(log)).stderr(Stdio::from(stderr));
        let process = command.spawn().map_err(|source| RunError::SpawnServer {
            name: name.clone(),
            server_bin: properties.server_bin.to_owned(),
            source,
        })?;
        info!(pid = process.id(), "Server node started");

        Ok(Self {
            server_properties: properties,
            client_cert_file,
            log_file,
            pid_file,
            endpoint_file,
            process: process.into(),
        })
    }

    pub fn wait_until_ready(&self) -> Result<(), RunError> {
        wait_until("server endpoint to accept TCP connections", || {
            let endpoint = self.endpoint().ok()?;
            TcpStream::connect(endpoint).ok()?;
            Some(())
        })
    }

    pub fn endpoint(&self) -> Result<String, RunError> {
        let endpoint = std::fs::read_to_string(&self.endpoint_file).map_err(|source| {
            RunError::ReadEndpoint {
                path: self.endpoint_file.clone(),
                source,
            }
        })?;
        let endpoint = endpoint.trim();
        if endpoint.is_empty() {
            Err(RunError::ReadEndpoint {
                path: self.endpoint_file.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "The endpoint file is empty",
                ),
            })
        } else {
            Ok(endpoint.to_owned())
        }
    }

    pub fn wait_for_log(&self, pattern: &str) -> Result<(), RunError> {
        wait_until(&format!("log containing {pattern:?}"), || {
            self.log_contents().contains(pattern).then_some(())
        })
    }

    pub fn wait_for_auth_code(&self) -> Result<String, RunError> {
        let auth_code = wait_until("auth code in gateway log", || {
            parse_auth_code(&self.log_contents())
        })?;
        if auth_code.is_empty() {
            return Err(RunError::EmptyAuthCode {
                log: self.log_contents(),
            });
        }
        return Ok(auth_code);
    }

    pub fn log_contents(&self) -> String {
        std::fs::read_to_string(&self.log_file).unwrap_or_default()
    }

    pub fn pid_file_contents(&self) -> String {
        std::fs::read_to_string(&self.pid_file).unwrap_or_default()
    }

    pub fn ensure_running(&self) -> Result<(), RunError> {
        if let Some(status) =
            self.process
                .borrow_mut()
                .try_wait()
                .map_err(|source| RunError::TryWait {
                    name: self.name.clone(),
                    source,
                })?
        {
            return Err(RunError::NodeExited {
                name: self.name.clone(),
                status,
                log: self.log_contents(),
            });
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<(), RunError> {
        let mut process = self.process.borrow_mut();
        if process
            .try_wait()
            .map_err(|source| RunError::TryWait {
                name: self.name.clone(),
                source,
            })?
            .is_none()
        {
            warn!(name = %self.name, pid = process.id(), "stopping server node");
            process.kill().map_err(|source| RunError::KillServer {
                name: self.name.clone(),
                source,
            })?;
        }
        let _ = process.wait();
        Ok(())
    }
}

impl Drop for Server {
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
