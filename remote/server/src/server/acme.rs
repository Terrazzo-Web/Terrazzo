#![cfg(feature = "acme")]

//! Configuration for integration with [Let's Encrypt](https://letsencrypt.org).

use std::ops::Deref;
use std::sync::Arc;

pub use instant_acme;
use instant_acme::AccountCredentials;
use instant_acme::AuthorizationStatus;
use instant_acme::ChallengeType;
use instant_acme::LetsEncrypt;
use instant_acme::OrderStatus;
use nameth::NamedEnumValues as _;
use nameth::NamedType as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use trz_gateway_common::certificate_info::CertificateInfo;
use trz_gateway_common::dynamic_config::DynamicConfig;
use trz_gateway_common::dynamic_config::has_diff::DiffArc;
use trz_gateway_common::dynamic_config::has_diff::DiffOption;

pub mod active_challenges;
pub mod certificate_config;
mod environment_serde;
mod get_certificate;
mod tests;

/// ACME configuration to generate certificates with [Let's Encrypt](https://letsencrypt.org).
#[nameth]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AcmeConfig {
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
    pub credentials: Arc<Option<AccountCredentials>>,

    /// Contact info used to register an account.
    ///
    /// Use "mailto:email@address.com" format.
    pub contact: String,

    /// The domain name to generate certificate.
    ///
    /// Routes from [ActiveChallenges](active_challenges::ActiveChallenges)
    /// must be available under port 80 for this domain name, to prove domain
    /// name ownership.
    pub domain: String,
    pub certificate: Option<CertificateInfo<String>>,
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum AcmeError {
    #[error("[{n}] {0}", n = self.name())]
    FromCredentials(instant_acme::Error),

    #[error("[{n}] {0}", n = self.name())]
    CreateAccount(instant_acme::Error),

    #[error("[{n}] {0}", n = self.name())]
    NewOrder(instant_acme::Error),

    #[error("[{n}] {0}", n = self.name())]
    Authorizations(instant_acme::Error),

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

    #[error("[{n}] The certificate is being provisioned", n = self.name())]
    Pending,

    #[error("[{n}] {0}", n = self.name())]
    Arc(Arc<Self>),
}

impl std::fmt::Debug for AcmeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(AcmeConfig::type_name())
            .field("environment", &self.environment)
            .field("credentials", &self.credentials.is_some())
            .field("contact", &self.contact)
            .field("domain", &self.domain)
            .finish()
    }
}

fn clone_account_credentials(credentials: &AccountCredentials) -> AccountCredentials {
    let credentials = serde_json::to_string(credentials).expect("Serialize credentials");
    serde_json::from_str(&credentials).expect("Deserialize credentials")
}

#[derive(Clone)]
pub struct DynamicAcmeConfig(Arc<DynamicConfig<DiffOption<DiffArc<AcmeConfig>>>>);

impl Deref for DynamicAcmeConfig {
    type Target = Arc<DynamicConfig<DiffOption<DiffArc<AcmeConfig>>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Arc<DynamicConfig<DiffOption<DiffArc<AcmeConfig>>>>> for DynamicAcmeConfig {
    fn from(value: Arc<DynamicConfig<DiffOption<DiffArc<AcmeConfig>>>>) -> Self {
        Self(value)
    }
}
