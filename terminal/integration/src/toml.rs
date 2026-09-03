use std::path::Path;
use trz_gateway_common::p2p::GOOGLE_STUN;

pub fn server_toml(pid_file: &Path, _port: u16, root_ca: &Path, trash: &Path) -> String {
    format!(
        r#"
[server]
host = "localhost"
ports = [0, 0]
terminal-shell = "echo \"Welcome to Test Environment\"; exec /bin/bash -i"
trash = "{trash}"
pidfile = "{pid_file}"
private_root_ca = "{root_ca}"
token_lifetime = "5m"
token_refresh = "4m 50s"
config_file_watcher = true
certificate_renewal_threshold = "30days"

[server.config_file_poll_strategy]
fixed = "1h"
	"#,
        pid_file = toml_path(pid_file),
        root_ca = toml_path(root_ca),
        trash = toml_path(trash),
    )
}

pub fn client_toml(
    pid_file: &Path,
    root_ca: &Path,
    root_ca_cert: &Path,
    client_cert: &Path,
    gateway_endpoint: &str,
    trash: &Path,
) -> String {
    format!(
        r#"
[server]
host = "localhost"
ports = [0, 0]
trash = "{trash}"
pidfile = "{pid_file}"
private_root_ca = "{root_ca}"
token_lifetime = "5m"
token_refresh = "4m 50s"
config_file_watcher = true
certificate_renewal_threshold = "30days"

[server.config_file_poll_strategy]
fixed = "1h"

[mesh]
client_name = "{client_name}"
gateway_url = "https://{gateway_endpoint}"
gateway_pki = "{root_ca_cert}"
client_certificate = "{client_cert}"
client_certificate_renewal = "30days"

[mesh.retry_strategy]
fixed = "1s"
"#,
        pid_file = toml_path(pid_file),
        root_ca = toml_path(root_ca),
        client_name = "test-client",
        gateway_endpoint = gateway_endpoint,
        root_ca_cert = toml_path(root_ca_cert),
        client_cert = toml_path(client_cert),
        trash = toml_path(trash),
    )
}

pub fn server_p2p_toml(
    pid_file: &Path,
    port: u16,
    root_ca: &Path,
    trash: &Path,
    signaling_endpoint: &str,
    server_name: &str,
) -> String {
    format!(
        r#"{}

[server.p2p_registration]
signaling_url = "http://{signaling_endpoint}"
server_name = "{server_name}"
handshake_timeout = "30s"
max_sessions = 8

[[server.p2p_registration.ice_servers]]
urls = ["{GOOGLE_STUN}"]

[server.p2p_registration.retry_strategy]
fixed = "250ms"
"#,
        server_toml(pid_file, port, root_ca, trash),
    )
}

pub fn client_p2p_toml(
    pid_file: &Path,
    root_ca: &Path,
    gateway_pki: &Path,
    client_cert: &Path,
    signaling_endpoint: &str,
    server_name: &str,
    trash: &Path,
) -> String {
    format!(
        r#"
[server]
host = "localhost"
ports = [0, 0]
trash = "{trash}"
pidfile = "{pid_file}"
private_root_ca = "{root_ca}"
token_lifetime = "5m"
token_refresh = "4m 50s"
config_file_watcher = true
certificate_renewal_threshold = "30days"

[server.config_file_poll_strategy]
fixed = "1h"

[mesh]
client_name = "test-client"
# This authority is used for target TLS only. It contains no target TCP port.
gateway_url = "https://localhost"
gateway_pki = "{gateway_pki}"
client_certificate = "{client_cert}"
client_certificate_renewal = "30days"

[mesh.web_rtc]
signaling_url = "http://{signaling_endpoint}"
server_name = "{server_name}"
signaling_timeout = "10s"
handshake_timeout = "30s"
connect_timeout = "45s"

[[mesh.web_rtc.ice_servers]]
urls = ["{GOOGLE_STUN}"]

[mesh.retry_strategy]
fixed = "1s"
"#,
        pid_file = toml_path(pid_file),
        root_ca = toml_path(root_ca),
        gateway_pki = toml_path(gateway_pki),
        client_cert = toml_path(client_cert),
        signaling_endpoint = signaling_endpoint,
        server_name = server_name,
        trash = toml_path(trash),
    )
}

fn toml_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}
