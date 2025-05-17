#![cfg(feature = "acme")]
#![allow(unused)]

use std::ops::ControlFlow;
use std::time::Duration;

use instant_acme::Account;
use instant_acme::AccountCredentials;
use instant_acme::AuthorizationStatus;
use instant_acme::ChallengeType;
use instant_acme::Identifier;
use instant_acme::LetsEncrypt;
use instant_acme::NewAccount;
use instant_acme::NewOrder;
use instant_acme::Order;
use instant_acme::OrderStatus;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use rcgen::CertificateParams;
use rcgen::DistinguishedName;
use rcgen::KeyPair;
use tokio::time::sleep;
use tracing::info;

pub struct AcmeConfig {
    environment: LetsEncrypt,
    credentials: Option<AccountCredentials>,
    contact: String,
    domain: String,
}

pub struct AcmeCertificate {
    pub certificate: String,
    pub private_key: String,
    pub credentials: Option<AccountCredentials>,
}

pub async fn get_certificate(config: AcmeConfig) -> Result<AcmeCertificate, AcmeError> {
    let (account, credentials) = if let Some(credentials) = config.credentials {
        let a = Account::from_credentials(credentials)
            .await
            .map_err(AcmeError::FromCredentials)?;
        (a, None)
    } else {
        let (a, c) = Account::create(
            &NewAccount {
                contact: &[&config.contact],
                terms_of_service_agreed: true,
                only_return_existing: false,
            },
            LetsEncrypt::Staging.url(),
            None,
        )
        .await
        .map_err(AcmeError::CreateAccount)?;
        (a, Some(c))
    };

    info!("Got account ID = {}", account.id());

    let identifier = Identifier::Dns(config.domain.clone());
    let mut order = account
        .new_order(&NewOrder {
            identifiers: &[identifier],
        })
        .await
        .map_err(AcmeError::NewOrder)?;

    let state = order.state();
    info!("Order created in state: {:?}", state);
    debug_assert!(matches!(state.status, OrderStatus::Pending));

    let authorizations = order
        .authorizations()
        .await
        .map_err(AcmeError::Authorizations)?;
    info!("Order has {} authorizations", authorizations.len());
    let mut challenges = Vec::with_capacity(authorizations.len());

    for authorization in &authorizations {
        match authorization.status {
            AuthorizationStatus::Pending => {}
            AuthorizationStatus::Valid => continue,
            AuthorizationStatus::Invalid
            | AuthorizationStatus::Revoked
            | AuthorizationStatus::Expired => {
                return Err(AcmeError::InvalidAuthorizationStatus(authorization.status));
            }
        }

        let challenge = authorization
            .challenges
            .iter()
            .find(|c| c.r#type == ChallengeType::Http01)
            .ok_or(AcmeError::Http01ChallengeMissing)?;
        info!("Found challenge {challenge:?}");

        let Identifier::Dns(identifier) = &authorization.identifier;
        info!("The identifier is {identifier}");

        // TODO: Serve the http01 challenge
        // http://pavy.one/.well-known/acme-challenge/{token} --> {key_authorization.as_str()}
        let token = challenge.token.as_str();
        let key_authorization = order.key_authorization(challenge);

        challenges.push(&challenge.url);
    }

    // Let the server know we're ready to accept the challenges.

    for url in &challenges {
        let () = order
            .set_challenge_ready(url)
            .await
            .map_err(AcmeError::SetChallengeReady)?;
    }
    info!("Set challenges as ready");

    // Exponentially back off until the order becomes ready or invalid.

    let status = poll(&mut order, 5, Duration::from_millis(250)).await?;
    info!("Order status: {:?}", status);

    // If the order is ready, we can provision the certificate.
    // Use the rcgen library to create a Certificate Signing Request.

    let mut params = CertificateParams::new(vec![config.domain])?;
    params.distinguished_name = DistinguishedName::new();
    let private_key = KeyPair::generate()?;
    let csr = params.serialize_request(&private_key)?;

    // Finalize the order and print certificate chain, private key and account credentials.

    let () = order
        .finalize(csr.der())
        .await
        .map_err(AcmeError::Finalize)?;
    let certificate = loop {
        match order.certificate().await.map_err(AcmeError::Certificate)? {
            Some(certificate) => break certificate,
            None => sleep(Duration::from_secs(1)).await,
        }
    };

    Ok(AcmeCertificate {
        certificate,
        private_key: private_key.serialize_pem(),
        credentials,
    })
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
}

async fn poll(order: &mut Order, tries: i32, delay: Duration) -> Result<OrderStatus, AcmeError> {
    let mut status = OrderStatus::Pending;
    if let Some(result) = poll2(
        async |last_status| {
            let state = match order.refresh().await.map_err(AcmeError::Refresh) {
                Ok(state) => state,
                Err(error) => return ControlFlow::Break(Err(error)),
            };
            *last_status = state.status;
            info!("Order is now in state: {last_status:?}");
            let result = match last_status {
                OrderStatus::Pending | OrderStatus::Processing => return ControlFlow::Continue(()),
                OrderStatus::Ready | OrderStatus::Valid => Ok(*last_status),
                OrderStatus::Invalid => Err(AcmeError::OrderFailed(*last_status)),
            };
            return ControlFlow::Break(result);
        },
        &mut status,
        tries,
        delay,
    )
    .await
    {
        return result;
    }

    return Err(AcmeError::OrderTimeout(status));
}

async fn poll2<S, R>(
    mut f: impl AsyncFnMut(&mut S) -> ControlFlow<R, ()>,
    state: &mut S,
    mut tries: i32,
    mut delay: Duration,
) -> Option<R> {
    while tries > 0 {
        tokio::time::sleep(delay).await;
        match f(state).await {
            ControlFlow::Continue(()) => (),
            ControlFlow::Break(result) => return Some(result),
        }
        delay *= 2;
        tries -= 1;
    }
    return None;
}

#[cfg(test)]
mod tests {
    use trz_gateway_common::crypto_provider::crypto_provider;
    use trz_gateway_common::tracing::test_utils::enable_tracing_for_tests;

    #[tokio::test]
    async fn get_certificate() {
        enable_tracing_for_tests();
        crypto_provider();
        let result = super::get_certificate(super::AcmeConfig {
            environment: instant_acme::LetsEncrypt::Staging,
            credentials: None,
            contact: "info@pavy.one".into(),
            domain: "pavy.one".into(),
        })
        .await;
        assert!(result.is_ok());
    }
}
