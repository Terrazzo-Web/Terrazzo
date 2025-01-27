use std::error::Error;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;

use openssl::asn1::Asn1Time;
use reqwest::header::CONTENT_TYPE;
use reqwest::StatusCode;
use tempfile::TempDir;
use tracing::debug;
use trz_gateway_common::tracing::test_utils::enable_tracing_for_tests;

use super::gateway_configuration::GatewayConfig;
use super::root_ca_configuration::RootCaConfigError;
use crate::auth_code::AuthCode;
use crate::security_configuration::certificate::CertificateConfig;
use crate::security_configuration::certificate::PemCertificate;
use crate::security_configuration::trusted_store::PemTrustedStore;
use crate::security_configuration::SecurityConfig;
use crate::server::certificate::GetCertificateRequest;
use crate::server::Server;
use crate::utils::x509::ca::make_intermediate;
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
        let trusted_root = Certificate::from_pem(
            config
                .tls_config
                .trusted_store
                .root_certificates_pem
                .as_bytes(),
        )?;
        reqwest::ClientBuilder::new()
            .add_root_certificate(trusted_root)
            .build()?
    };
    let mut wait = Duration::from_millis(1);
    while wait < Duration::from_secs(5) {
        let request = client.get(format!("https://{}:{}/status", config.host(), config.port));
        match request.send().await {
            Ok(response) => match response.text().await.as_deref() {
                Ok("UP") => return Ok(client),
                response => debug!("Unexpected response: {response:?}"),
            },
            Err(error) => debug!("Failed: {error:?}"),
        }
        tokio::time::sleep(wait).await;
        wait = wait * 2;
    }
    panic!("Failed to connect")
}

#[derive(Debug)]

struct TestConfig {
    port: u16,
    root_ca_config: Arc<PemCertificate>,
    tls_config: Arc<SecurityConfig<PemTrustedStore, PemCertificate>>,
}

impl TestConfig {
    fn new() -> Arc<Self> {
        enable_tracing_for_tests();
        let root_ca_config = root_ca_config().expect("root_ca_config()").into();
        let tls_config = tls_config().expect("tls_config()").into();
        Arc::new(Self {
            port: portpicker::pick_unused_port().expect("pick_unused_port()"),
            root_ca_config,
            tls_config,
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

    type RootCaConfig = Arc<PemCertificate>;
    fn root_ca(&self) -> Self::RootCaConfig {
        self.root_ca_config.clone()
    }

    type TlsConfig = Arc<SecurityConfig<PemTrustedStore, PemCertificate>>;
    fn tls(&self) -> Self::TlsConfig {
        self.tls_config.clone()
    }

    type ClientCertificateIssuerConfig = Arc<SecurityConfig<PemTrustedStore, PemCertificate>>;
    fn client_certificate_issuer(&self) -> Self::ClientCertificateIssuerConfig {
        self.tls_config.clone()
    }
}

fn root_ca_config() -> Result<PemCertificate, RootCaConfigError> {
    static MUTEX: std::sync::Mutex<()> = Mutex::new(());
    let lock = MUTEX.lock().unwrap();
    let tempdir = temp_dir();
    let root_ca = PemCertificate::load_root_ca(
        "Test Root CA".to_owned(),
        tempdir.path().join(ROOT_CA_CERTIFICATE_FILENAME),
        tempdir.path().join(ROOT_CA_PRIVATE_KEY_FILENAME),
        Validity { from: 0, to: 365 }
            .try_map(Asn1Time::days_from_now)
            .expect("Asn1Time::days_from_now")
            .as_deref()
            .try_into()
            .expect("Asn1Time to SystemTime"),
    )?;
    drop(lock);
    Ok(root_ca)
}

fn tls_config() -> Result<SecurityConfig<PemTrustedStore, PemCertificate>, Box<dyn Error>> {
    let root_ca_config = root_ca_config()?;
    let root_ca = root_ca_config.certificate()?;
    let root_certificate_pem = root_ca_config.certificate_pem;
    let validity = root_ca.certificate.as_ref().try_into()?;

    let (intermediate, intermediate_key) = make_intermediate(
        &root_ca.certificate,
        &root_ca.private_key,
        CertitficateName {
            organization: Some("Terrazzo Test"),
            common_name: Some("Intermediate CA"),
            ..CertitficateName::default()
        },
        validity,
    )?;

    let certificate_key = make_key()?;
    let certificate = make_cert(
        &intermediate,
        &intermediate_key,
        CertitficateName {
            organization: Some("Terrazzo Test"),
            common_name: Some("localhost"),
            ..CertitficateName::default()
        },
        validity,
        &certificate_key.public_key_to_pem().pem_string()?,
        vec![],
    )?;

    Ok(SecurityConfig {
        trusted_store: PemTrustedStore {
            root_certificates_pem: root_certificate_pem,
        },
        certificate: PemCertificate {
            intermediates_pem: intermediate.to_pem()?.pem_string()?,
            certificate_pem: certificate.to_pem()?.pem_string()?,
            private_key_pem: certificate_key.private_key_to_pem_pkcs8()?.pem_string()?,
        },
    })
}

fn temp_dir() -> &'static TempDir {
    static CONFIG: OnceLock<TempDir> = OnceLock::new();
    CONFIG.get_or_init(|| {
        TempDir::new()
            .inspect(|temp_dir| debug!("Using tempprary folder {}", temp_dir.path().display()))
            .expect("TempDir::new()")
    })
}
