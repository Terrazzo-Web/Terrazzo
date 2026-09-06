#![cfg(feature = "acme")]

//! Configuration for integration with [Let's Encrypt](https://letsencrypt.org).

use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

pub use instant_acme;
use instant_acme::AccountCredentials;
use instant_acme::AuthorizationStatus;
use instant_acme::ChallengeType;
use instant_acme::LetsEncrypt;
use instant_acme::OrderStatus;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use trz_gateway_common::dynamic_config::DynamicConfig;
use trz_gateway_common::dynamic_config::has_diff::DiffArc;
use trz_gateway_common::dynamic_config::has_diff::DiffOption;

pub mod active_challenges;
pub mod certificate_config;
mod domains_serde;
mod environment_serde;
mod get_certificate;
mod tests;

/// ACME configuration to generate certificates with [Let's Encrypt](https://letsencrypt.org).
#[nameth]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AcmeConfig<P = PathBuf> {
    /// Use [Production](LetsEncrypt::Production) or [Staging](LetsEncrypt::Staging)
    #[serde(with = "environment_serde")]
    pub environment: LetsEncrypt,

    /// Let's Encrypt credentials.
    ///
    /// An account is automatically created and added to configuration if necessary.
    ///
    /// Certificates are implemented using [AcmeCertificateConfig](certificate_config::AcmeCertificateConfig)
    /// based on a [DynamicAcmeConfig].
    ///
    /// The dynamic configuration is updated with the account credentials when
    /// the certificate generation logic runs.
    #[serde(default)]
    pub credentials: Arc<Option<AccountCredentials>>,

    /// Contact info used to register an account.
    ///
    /// Use "mailto:email@address.com" format.
    pub contact: String,

    /// The domain names to generate certificate.
    ///
    /// Routes from [ActiveChallenges](active_challenges::ActiveChallenges)
    /// must be available under port 80 for these domain names, to prove domain
    /// name ownership.
    #[serde(alias = "domain", with = "domains_serde")]
    pub domains: Vec<String>,

    /// The generated certificate chain.
    pub certificate: Option<String>,

    /// The file where the generated private key is stored.
    pub private_key: P,
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum AcmeError {
    #[error("[{n}] {0}", n = self.name())]
    Builder(instant_acme::Error),

    #[error("[{n}] {0}", n = self.name())]
    FromCredentials(instant_acme::Error),

    #[error("[{n}] {0}", n = self.name())]
    CreateAccount(instant_acme::Error),

    #[error("[{n}] {0}", n = self.name())]
    NewOrder(instant_acme::Error),

    #[error("[{n}] {0}", n = self.name())]
    Authorization(instant_acme::Error),

    #[error("[{n}] {0:?}", n = self.name())]
    InvalidAuthorizationStatus(AuthorizationStatus),

    #[error("[{n}] Challenge for '{c:?}' not found", c = ChallengeType::Http01, n = self.name())]
    Http01ChallengeMissing,

    #[error("[{n}] {0:?}", n = self.name())]
    SetChallengeReady(instant_acme::Error),

    #[error("[{n}] {0}", n = self.name())]
    Finalize(instant_acme::Error),

    #[error("[{n}] {0}", n = self.name())]
    Certificate(instant_acme::Error),

    #[error("[{n}] {0}", n = self.name())]
    Refresh(instant_acme::Error),

    #[error("[{n}] The order timed suck in '{0:?}'", n = self.name())]
    OrderTimeout(OrderStatus),

    #[error("[{n}] The order failed in status '{0:?}'", n = self.name())]
    OrderFailed(OrderStatus),

    #[error("[{n}] {0}", n = self.name())]
    CertificateGeneration(#[from] rcgen::Error),

    #[error("[{n}] The certificate chain was not valid", n = self.name())]
    CertificateChain,

    #[error("[{n}] The certificate chain was not valid", n = self.name())]
    OpenSSL(#[from] ErrorStack),

    #[error("[{n}] Failed to read private key {path:?}: {source}", n = self.name())]
    ReadPrivateKey {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("[{n}] Failed to write private key {path:?}: {source}", n = self.name())]
    WritePrivateKey {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("[{n}] The certificate is being provisioned", n = self.name())]
    Pending,

    #[error("[{n}] {0}", n = self.name())]
    Arc(Arc<Self>),

    #[error("[{n}] Unexpected identifier format", n = self.name())]
    UnexpectedIdentifierFormat,

    #[error("[{n}] {0}", n = self.name())]
    SetReady(instant_acme::Error),
}

impl<P: PartialEq> PartialEq for AcmeConfig<P> {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self.environment, other.environment),
            (LetsEncrypt::Production, LetsEncrypt::Production)
                | (LetsEncrypt::Staging, LetsEncrypt::Staging)
        ) && credentials_eq(&self.credentials, &other.credentials)
            && self.contact == other.contact
            && self.domains == other.domains
            && self.certificate == other.certificate
            && self.private_key == other.private_key
    }
}

impl<P: Eq> Eq for AcmeConfig<P> {}

fn credentials_eq(a: &Option<AccountCredentials>, b: &Option<AccountCredentials>) -> bool {
    match (a, b) {
        (None, None) => true,
        (None, Some(_)) | (Some(_), None) => false,
        (Some(a), Some(b)) => {
            if let (Ok(a), Ok(b)) = (serde_json::to_string(a), serde_json::to_string(b)) {
                a == b
            } else {
                false
            }
        }
    }
}

impl<P: std::fmt::Debug> std::fmt::Debug for AcmeConfig<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(ACME_CONFIG)
            .field("environment", &self.environment)
            .field("credentials", &self.credentials.is_some())
            .field("contact", &self.contact)
            .field("domains", &self.domains)
            .field("private_key", &self.private_key)
            .finish()
    }
}

impl<P: AsRef<Path>> AcmeConfig<P> {
    fn private_key_path(&self) -> &Path {
        self.private_key.as_ref()
    }
}

fn clone_account_credentials(credentials: &AccountCredentials) -> AccountCredentials {
    let credentials = serde_json::to_string(credentials).expect("Serialize credentials");
    serde_json::from_str(&credentials).expect("Deserialize credentials")
}

#[derive(Clone)]
pub struct DynamicAcmeConfig<P = PathBuf>(Arc<DynamicConfig<DiffOption<DiffArc<AcmeConfig<P>>>>>);

impl<P> Deref for DynamicAcmeConfig<P> {
    type Target = Arc<DynamicConfig<DiffOption<DiffArc<AcmeConfig<P>>>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<P> From<Arc<DynamicConfig<DiffOption<DiffArc<AcmeConfig<P>>>>>> for DynamicAcmeConfig<P> {
    fn from(value: Arc<DynamicConfig<DiffOption<DiffArc<AcmeConfig<P>>>>>) -> Self {
        Self(value)
    }
}
