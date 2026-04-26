use std::cell::RefCell;
use std::ffi::OsString;
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;

use tracing::info;
use tracing::warn;

use crate::RunError;
use crate::wait_until;

pub struct ServerInstance {
    name: String,
    child: RefCell<Option<Child>>,
    endpoint_file: PathBuf,
    log_file: PathBuf,
}

impl ServerInstance {
    pub fn start(
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

    pub fn wait_until_ready(&self) -> Result<(), RunError> {
        wait_until("server endpoint to accept TCP connections", || {
            let endpoint = self.endpoint().ok()?;
            TcpStream::connect(endpoint).ok()?;
            Some(())
        })
    }

    pub fn endpoint(&self) -> Result<String, RunError> {
        Ok(std::fs::read_to_string(&self.endpoint_file)
            .map_err(|source| RunError::ReadEndpoint {
                path: self.endpoint_file.clone(),
                source,
            })?
            .trim()
            .to_owned())
    }

    pub fn wait_for_log(&self, pattern: &str) -> Result<(), RunError> {
        wait_until(&format!("log containing {pattern:?}"), || {
            self.log_contents().contains(pattern).then_some(())
        })
    }

    pub fn wait_for_auth_code(&self) -> Result<String, RunError> {
        wait_until("auth code in gateway log", || {
            parse_auth_code(&self.log_contents())
        })
    }

    pub fn log_contents(&self) -> String {
        std::fs::read_to_string(&self.log_file).unwrap_or_default()
    }

    pub fn ensure_running(&self) -> Result<(), RunError> {
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

    pub fn stop(&self) -> Result<(), RunError> {
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
