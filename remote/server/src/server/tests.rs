use std::error::Error;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use openssl::asn1::Asn1Time;
use reqwest::header::CONTENT_TYPE;
use reqwest::StatusCode;
use tempfile::TempDir;

use super::gateway_configuration::GatewayConfig;
use super::root_ca_configuration::RootCaConfig;
use crate::auth_code::AuthCode;
use crate::security_configuration::SecurityConfig;
use crate::server::certificate::GetCertificateRequest;
use crate::server::Server;
use crate::utils::x509::cert::make_cert;
use crate::utils::x509::key::make_key;
use crate::utils::x509::name::CertitficateName;
use crate::utils::x509::validity::Validity;
use crate::utils::x509::PemString as _;

const ROOT_CA_CERTIFICATE_FILENAME: &str = "root-ca-cert.pem";
const ROOT_CA_PRIVATE_KEY_FILENAME: &str = "root-ca-key.pem";

#[tokio::test]
async fn status() -> Result<(), Box<dyn Error>> {
    let config = TestConfig::new();
    let (shutdown, terminated) = Server::run(config.clone()).await?;

    let _client = make_client(&config).await;

    shutdown.send("End of test".into())?;
    let () = terminated.await?;
    Ok(())
}

#[tokio::test]
async fn certificate() -> Result<(), Box<dyn Error>> {
    let config = TestConfig::new();
    let (shutdown, terminated) = Server::run(config.clone()).await.expect("Server::run");

    let client = make_client(&config).await?;

    let private_key = make_key()?;
    let public_key = private_key.public_key_to_pem().pem_string()?;

    let request = client
        .get(format!(
            "https://{}:{}/remote/certificate",
            config.host(),
            config.port
        ))
        .header(CONTENT_TYPE, "application/json")
        .body(serde_json::to_string(&GetCertificateRequest {
            code: AuthCode::current(),
            public_key,
            name: "Test cert".to_owned(),
        })?);
    let response = request.send().await?;
    assert_eq!(StatusCode::OK, response.status());

    let body = response.text().await?;
    let (rest, certificate) = x509_parser::pem::parse_x509_pem(body.as_bytes())?;
    assert_eq!([0; 0], rest);
    let certificate = certificate.parse_x509()?;
    assert_eq!("CN=Test Root CA", certificate.issuer().to_string());
    assert_eq!("CN=Test cert", certificate.subject().to_string());

    shutdown.send("End of test".into())?;
    let () = terminated.await?;
    Ok(())
}

#[tokio::test]
async fn invalid_auth_code() -> Result<(), Box<dyn Error>> {
    let config = TestConfig::new();
    let (shutdown, terminated) = Server::run(config.clone()).await?;

    let client = make_client(&config).await?;

    let private_key = make_key()?;
    let public_key = private_key.public_key_to_pem().pem_string()?;

    let request = client
        .get(format!(
            "https://{}:{}/remote/certificate",
            config.host(),
            config.port
        ))
        .header(CONTENT_TYPE, "application/json")
        .body(serde_json::to_string(&GetCertificateRequest {
            code: AuthCode::from("invalid-code"),
            public_key,
            name: "Test cert".to_owned(),
        })?);
    let response = request.send().await?;
    assert_eq!(StatusCode::FORBIDDEN, response.status());

    let body = response.text().await?;
    assert_eq!("[InvalidAuthCode] AuthCode is invalid", body);

    shutdown.send("End of test".into())?;
    let () = terminated.await?;
    Ok(())
}

async fn make_client(config: &TestConfig) -> Result<reqwest::Client, Box<dyn Error>> {
    let client = {
        use reqwest::tls::Certificate;
        let root_ca = Certificate::from_pem(config.root_ca_config.certificate_pem().as_bytes())?;
        reqwest::ClientBuilder::new()
            .add_root_certificate(root_ca)
            .build()?
    };
    let mut wait = Duration::from_millis(1);
    while wait < Duration::from_secs(5) {
        let request = client.get(format!("https://{}:{}/status", config.host(), config.port));
        if let Ok(response) = request.send().await {
            if let Ok("UP") = response.text().await.as_deref() {
                return Ok(client);
            }
        }
        tokio::time::sleep(wait).await;
        wait = wait * 2;
    }
    panic!("Failed to connect")
}

fn root_ca_config() -> &'static Result<Arc<RootCaConfig>, Box<dyn Error + Send + Sync + 'static>> {
    static CONFIG: OnceLock<Result<Arc<RootCaConfig>, Box<dyn Error + Send + Sync + 'static>>> =
        OnceLock::new();
    CONFIG.get_or_init(|| {
        let tempdir = TempDir::new()?;
        Ok(RootCaConfig::load(
            "Test Root CA".to_owned(),
            tempdir.path().join(ROOT_CA_CERTIFICATE_FILENAME),
            tempdir.path().join(ROOT_CA_PRIVATE_KEY_FILENAME),
            Validity { from: 0, to: 365 }
                .try_map(Asn1Time::days_from_now)?
                .as_deref()
                .try_into()?,
        )?
        .into())
    })
}

struct TestConfig {
    port: u16,
    root_ca_config: Arc<RootCaConfig>,
}

impl TestConfig {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            port: portpicker::pick_unused_port().expect("pick_unused_port()"),
            root_ca_config: root_ca_config().as_ref().unwrap().clone(),
        })
    }
}

impl GatewayConfig for TestConfig {
    fn enable_tracing(&self) -> bool {
        false
    }

    fn host(&self) -> &str {
        "localhost"
    }

    fn port(&self) -> u16 {
        self.port
    }

    type RootCaConfig = Arc<RootCaConfig>;
    fn root_ca(&self) -> Self::RootCaConfig {
        self.root_ca_config.clone()
    }

    type TlsConfig = TestTlsConfig;
    fn tls(&self) -> Self::TlsConfig {
        let key = make_key().unwrap();
        let root_ca = self.root_ca_config.certificate().unwrap();
        let cert = make_cert(
            &root_ca.certificate,
            &root_ca.private_key,
            CertitficateName {
                organization: Some("Test terrazzo"),
                common_name: Some("localhost"),
                ..CertitficateName::default()
            },
            root_ca.certificate.as_ref().try_into().unwrap(),
            &key.public_key_to_pem().pem_string().unwrap(),
            vec![],
        )
        .unwrap();
        TestTlsConfig {
            certificate: cert.to_pem().pem_string().unwrap(),
            private_key: key.private_key_to_pem_pkcs8().pem_string().unwrap(),
        }
    }
}

struct TestTlsConfig {
    certificate: String,
    private_key: String,
}
impl SecurityConfig for TestTlsConfig {
    fn certificate_pem(&self) -> &str {
        &self.certificate
    }
    fn private_key_pem(&self) -> &str {
        &self.private_key
    }
}
