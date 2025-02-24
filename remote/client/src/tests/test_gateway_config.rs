use std::error::Error;
use std::sync::Arc;
use std::sync::Mutex;

use openssl::asn1::Asn1Time;
use tempfile::TempDir;
use terrazzo_fixture::fixture::Fixture;
use tracing::debug;
use trz_gateway_common::certificate_info::CertificateInfo;
use trz_gateway_common::security_configuration::SecurityConfig;
use trz_gateway_common::security_configuration::certificate::CertificateConfig as _;
use trz_gateway_common::security_configuration::certificate::pem::PemCertificate;
use trz_gateway_common::security_configuration::trusted_store::pem::PemTrustedStore;
use trz_gateway_common::tracing::test_utils::enable_tracing_for_tests;
use trz_gateway_common::x509::PemString as _;
use trz_gateway_common::x509::ca::make_intermediate;
use trz_gateway_common::x509::cert::make_cert;
use trz_gateway_common::x509::key::make_key;
use trz_gateway_common::x509::name::CertitficateName;
use trz_gateway_common::x509::validity::Validity;
use trz_gateway_server::server::gateway_config::GatewayConfig;
use trz_gateway_server::server::root_ca_configuration;
use trz_gateway_server::server::root_ca_configuration::RootCaConfigError;

const ROOT_CA_FILENAME: CertificateInfo<&str> = CertificateInfo {
    certificate: "root-ca-cert.pem",
    private_key: "root-ca-key.pem",
};

#[derive(Debug)]
pub struct TestGatewayConfig {
    port: u16,
    root_ca: Arc<PemCertificate>,
    tls_config: <Self as GatewayConfig>::TlsConfig,
}

impl TestGatewayConfig {
    pub fn new() -> Arc<Self> {
        enable_tracing_for_tests();
        let root_ca = make_root_ca().expect("test_root_ca()");
        let tls_config = make_tls_config().expect("tls_config()");
        Arc::new(Self {
            port: portpicker::pick_unused_port().expect("pick_unused_port()"),
            root_ca,
            tls_config,
        })
    }
}

impl GatewayConfig for TestGatewayConfig {
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
        self.root_ca.clone()
    }

    type TlsConfig = Arc<SecurityConfig<Arc<PemTrustedStore>, PemCertificate>>;
    fn tls(&self) -> Self::TlsConfig {
        // The TLS server certificate is the same as the signing certificate.
        self.tls_config.clone()
    }

    type ClientCertificateIssuerConfig = Self::TlsConfig;
    fn client_certificate_issuer(&self) -> Self::ClientCertificateIssuerConfig {
        // The signing certificate is the same as the TLS server certificate.
        self.tls_config.clone()
    }
}

fn make_root_ca() -> Result<Arc<PemCertificate>, RootCaConfigError> {
    let temp_dir = TEMP_DIR.get();

    static MUTEX: std::sync::Mutex<()> = Mutex::new(());
    let _lock = MUTEX.lock().unwrap();
    let root_ca = root_ca_configuration::load_root_ca(
        "Test Root CA".to_owned(),
        ROOT_CA_FILENAME.map(|filename| temp_dir.path().join(filename)),
        Validity { from: 0, to: 365 }
            .try_map(Asn1Time::days_from_now)
            .expect("Asn1Time::days_from_now")
            .as_deref()
            .try_into()
            .expect("Asn1Time to SystemTime"),
    )?;
    Ok(Arc::new(root_ca))
}

fn make_tls_config() -> Result<<TestGatewayConfig as GatewayConfig>::TlsConfig, Box<dyn Error>> {
    let root_ca_config = make_root_ca()?;
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

    Ok(Arc::new(SecurityConfig {
        trusted_store: Arc::new(PemTrustedStore {
            root_certificates_pem: root_certificate_pem,
        }),
        certificate: PemCertificate {
            intermediates_pem: intermediate.certificate.to_pem()?.pem_string()?,
            certificate_pem: certificate.to_pem()?.pem_string()?,
            private_key_pem: certificate_key.private_key_to_pem_pkcs8()?.pem_string()?,
        },
    }))
}

static TEMP_DIR: Fixture<TempDir> = Fixture::new();

pub fn use_temp_dir() -> Arc<TempDir> {
    TEMP_DIR.get_or_init(|| {
        TempDir::new()
            .inspect(|temp_dir| debug!("Using temporary folder {}", temp_dir.path().display()))
            .expect("TempDir::new()")
    })
}
