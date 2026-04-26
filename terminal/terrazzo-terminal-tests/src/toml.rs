use std::path::Path;

pub fn server_toml(pid_file: &Path, port: u16, root_ca: &Path) -> String {
    format!(
        r#"
[server]
host = "127.0.0.1"
port = {port}
pidfile = "{pid_file}"
private_root_ca = "{root_ca}"
token_lifetime = "5m"
token_refresh = "4m 50s"
config_file_watcher = false
certificate_renewal_threshold = "30days"
"#,
        pid_file = toml_path(pid_file),
        root_ca = toml_path(root_ca),
    )
}

pub fn client_toml(
    pid_file: &Path,
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
pidfile = "{pid_file}"
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
        pid_file = toml_path(pid_file),
        root_ca = toml_path(root_ca),
        client_name = "test-client",
        gateway_endpoint = gateway_endpoint,
        root_ca_cert = toml_path(root_ca_cert),
        client_cert = toml_path(client_cert),
    )
}

fn toml_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}
