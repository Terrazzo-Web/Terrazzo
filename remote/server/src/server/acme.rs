#![cfg(feature = "acme")]

use std::sync::Arc;

use instant_acme::AccountCredentials;
use instant_acme::AuthorizationStatus;
use instant_acme::ChallengeType;
use instant_acme::LetsEncrypt;
use instant_acme::OrderStatus;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;

mod certificate_config;
mod environment_serde;
mod get_certificate;
mod tests;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct AcmeConfig {
    #[serde(with = "environment_serde")]
    environment: LetsEncrypt,
    credentials: Option<AccountCredentials>,
    contact: String,
    domain: String,
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
        f.debug_struct("AcmeConfig")
            .field("environment", &self.environment)
            .field("credentials", &self.credentials.is_some())
            .field("contact", &self.contact)
            .field("domain", &self.domain)
            .finish()
    }
}

impl Clone for AcmeConfig {
    fn clone(&self) -> Self {
        let credentials = self.credentials.as_ref().map(|credentials| {
            let credentials = serde_json::to_string(credentials).expect("Serialize credentials");
            serde_json::from_str(&credentials).expect("Deserialize credentials")
        });
        Self {
            environment: self.environment,
            credentials,
            contact: self.contact.clone(),
            domain: self.domain.clone(),
        }
    }
}
