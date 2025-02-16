use std::error::Error;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use mime::APPLICATION_JSON;
use openssl::asn1::Asn1Time;
use openssl::pkey::HasPublic;
use openssl::pkey::PKeyRef;
use reqwest::header::CONTENT_TYPE;
use reqwest::Response;
use reqwest::StatusCode;
use tempfile::TempDir;
use terrazzo_testutils::fixture::Fixture;
use tracing::debug;
use trz_gateway_common::api::tunnel::GetCertificateRequest;
use trz_gateway_common::certificate_info::CertificateInfo;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificate;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::security_configuration::trusted_store::pem::PemTrustedStore;
use trz_gateway_common::security_configuration::SecurityConfig;
use trz_gateway_common::tracing::test_utils::enable_tracing_for_tests;
use trz_gateway_common::x509::ca::make_intermediate;
use trz_gateway_common::x509::cert::make_cert;
use trz_gateway_common::x509::key::make_key;
use trz_gateway_common::x509::name::CertitficateName;
use trz_gateway_common::x509::validity::Validity;
use trz_gateway_common::x509::PemString as _;

use super::gateway_config::GatewayConfig;
use super::root_ca_configuration;
use super::root_ca_configuration::RootCaConfigError;
use super::Server;
use crate::auth_code::AuthCode;

const ROOT_CA_FILENAME: CertificateInfo<&str> = CertificateInfo {
    certificate: "root-ca-cert.pem",
    private_key: "root-ca-key.pem",
};

#[tokio::test]
async fn status() -> Result<(), Box<dyn Error>> {
    let config = TestConfig::new();
    let handle = Server::run(config.clone()).await?;

    let _client = make_client(&config).await?;

    let () = handle.stop("End of test").await?;
    Ok(())
}

#[tokio::test]
async fn certificate() -> Result<(), Box<dyn Error>> {
    let config = TestConfig::new();
    let handle = Server::run(config.clone()).await?;

    let client = make_client(&config).await?;

    let private_key = make_key()?;
    let response = send_certificate_request(&config, client, &private_key).await?;
    assert_eq!(StatusCode::OK, response.status());

    let pem = response.text().await?;
    let (rest, certificate) = x509_parser::pem::parse_x509_pem(pem.as_bytes())?;
    assert_eq!([0; 0], rest);
    let certificate = certificate.parse_x509()?;
    assert_eq!("CN=Test Root CA", certificate.issuer().to_string());
    assert_eq!("CN=Test cert", certificate.subject().to_string());

    let () = handle.stop("End of test").await?;
    Ok(())
}

#[tokio::test]
async fn invalid_auth_code() -> Result<(), Box<dyn Error>> {
    let config = TestConfig::new();
    let handle = Server::run(config.clone()).await?;

    let client = make_client(&config).await?;

    let private_key = make_key()?;
    let public_key = private_key.public_key_to_pem().pem_string()?;

    let request = client
        .get(format!(
            "https://{}:{}/remote/certificate",
            config.host(),
            config.port
        ))
        .header(CONTENT_TYPE, APPLICATION_JSON.as_ref())
        .body(serde_json::to_string(&GetCertificateRequest {
            auth_code: AuthCode::from("invalid-code"),
            public_key,
            name: "Test cert".into(),
        })?);
    let response = request.send().await?;
    assert_eq!(StatusCode::FORBIDDEN, response.status());

    let body = response.text().await?;
    assert_eq!("[InvalidAuthCode] AuthCode is invalid", body);

    let () = handle.stop("End of test").await?;
    Ok(())
}

#[tokio::test]
async fn tunnel() -> Result<(), Box<dyn Error>> {
    let config = TestConfig::new();
    let handle = Server::run(config.clone()).await?;

    let client = make_client(&config).await?;

    let private_key = make_key()?;
    let response = send_certificate_request(&config, client, &private_key).await?;
    assert_eq!(StatusCode::OK, response.status());

    let _pem = response.text().await?;

    let () = handle.stop("End of test").await?;
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
        let t = Instant::now();
        let request = client.get(format!("https://{}:{}/status", config.host(), config.port));
        match request.send().await {
            Ok(response) => match response.text().await.as_deref() {
                Ok("UP") => return Ok(client),
                response => debug!("Unexpected response: {response:?}"),
            },
            Err(error) => debug!("Failed: {error:?}"),
        }
        tokio::time::sleep(wait).await;
        wait = Duration::max(t.elapsed(), wait) * 2;
    }
    panic!("Failed to connect")
}

async fn send_certificate_request(
    config: &TestConfig,
    client: reqwest::Client,
    private_key: &PKeyRef<impl HasPublic>,
) -> Result<Response, Box<dyn Error>> {
    let public_key = private_key.public_key_to_pem().pem_string()?;
    let request = client
        .get(format!(
            "https://{}:{}/remote/certificate",
            config.host(),
            config.port
        ))
        .header(CONTENT_TYPE, APPLICATION_JSON.as_ref())
        .body(serde_json::to_string(&GetCertificateRequest {
            auth_code: AuthCode::current(),
            public_key,
            name: "Test cert".into(),
        })?);
    Ok(request.send().await?)
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

fn root_ca_config() -> Result<Arc<PemCertificate>, RootCaConfigError> {
    let tempdir = temp_dir();

    static MUTEX: std::sync::Mutex<()> = Mutex::new(());
    let _lock = MUTEX.lock().unwrap();
    let root_ca = root_ca_configuration::load_root_ca(
        "Test Root CA".to_owned(),
        ROOT_CA_FILENAME.map(|filename| tempdir.path().join(filename)),
        Validity { from: 0, to: 365 }
            .try_map(Asn1Time::days_from_now)
            .expect("Asn1Time::days_from_now")
            .as_deref()
            .try_into()
            .expect("Asn1Time to SystemTime"),
    )?;
    Ok(Arc::new(root_ca))
}

fn tls_config() -> Result<SecurityConfig<PemTrustedStore, PemCertificate>, Box<dyn Error>> {
    let root_ca_config = root_ca_config()?;
    let root_ca = root_ca_config.certificate()?;
    let root_certificate_pem = root_ca_config.certificate_pem.clone();
    let validity = root_ca.certificate.as_ref().try_into()?;

    let intermediate = make_intermediate(
        (*root_ca).as_ref(),
        CertitficateName {
            organization: Some("Terrazzo Test"),
            common_name: Some("Intermediate CA"),
            ..CertitficateName::default()
        },
        validity,
    )?;

    let certificate_key = make_key()?;
    let certificate = make_cert(
        intermediate.as_ref(),
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
            intermediates_pem: intermediate.certificate.to_pem()?.pem_string()?,
            certificate_pem: certificate.to_pem()?.pem_string()?,
            private_key_pem: certificate_key.private_key_to_pem_pkcs8()?.pem_string()?,
        },
    })
}

fn temp_dir() -> Arc<TempDir> {
    static FIXTURE: Fixture<TempDir> = Fixture::new();
    FIXTURE.get_or_init(|| {
        TempDir::new()
            .inspect(|temp_dir| debug!("Using tempprary folder {}", temp_dir.path().display()))
            .expect("TempDir::new()")
    })
}
