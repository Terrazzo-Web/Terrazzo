#![cfg(feature = "acme")]

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
use tracing::debug;
use tracing::info;

pub struct AcmeConfig {
    environment: LetsEncrypt,
    credentials: Option<AccountCredentials>,
    contact: String,
    domain: String,
}

async fn get_certificate(config: AcmeConfig) -> Result<(), AcmeError> {
    let (account, credentials) = if let Some(credentials) = config.credentials {
        let a = Account::from_credentials(credentials)
            .await
            .map_err(AcmeError::FromCredentials)?;
        (a, None)
    } else {
        let (a, c) = Account::create(
            &NewAccount {
                contact: &[],
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

    let identifier = Identifier::Dns(config.domain.clone());
    let mut order = account
        .new_order(&NewOrder {
            identifiers: &[identifier],
        })
        .await
        .map_err(AcmeError::NewOrder)?;

    let state = order.state();
    debug!("Order state: {:?}", state);
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

        // We'll use the DNS challenges for this example, but you could
        // pick something else to use here.

        let challenge = authorization
            .challenges
            .iter()
            .find(|c| c.r#type == ChallengeType::Http01)
            .ok_or(AcmeError::Http01ChallengeMissing)?;

        let Identifier::Dns(identifier) = &authorization.identifier;

        debug!("The identifier is {identifier}");
        // TODO: Serve the http01 challenge

        challenges.push(&challenge.url);
    }

    // Let the server know we're ready to accept the challenges.

    for url in &challenges {
        let () = order
            .set_challenge_ready(url)
            .await
            .map_err(AcmeError::SetChallengeReady)?;
    }

    // Exponentially back off until the order becomes ready or invalid.

    let status = poll(&mut order, 5, Duration::from_millis(250)).await?;
    debug!("Order status: {:?}", status);

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
    let cert_chain_pem = loop {
        match order.certificate().await.map_err(AcmeError::Certificate)? {
            Some(cert_chain_pem) => break cert_chain_pem,
            None => sleep(Duration::from_secs(1)).await,
        }
    };

    info!("certficate chain:\n\n{}", cert_chain_pem);
    info!("private key:\n\n{}", private_key.serialize_pem());
    Ok(())
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

async fn poll(
    order: &mut Order,
    mut tries: i32,
    mut delay: Duration,
) -> Result<OrderStatus, AcmeError> {
    let mut status = OrderStatus::Pending;
    while tries > 0 {
        tokio::time::sleep(delay).await;
        let state = order.refresh().await.map_err(AcmeError::Refresh)?;
        status = state.status;
        debug!("Order is now in state: {status:?}");
        match status {
            OrderStatus::Pending => (),
            OrderStatus::Ready | OrderStatus::Valid => return Ok(status),
            OrderStatus::Processing => (),
            OrderStatus::Invalid => return Err(AcmeError::OrderFailed(status)),
        }

        delay *= 2;
        tries -= 1;
    }
    return Err(AcmeError::OrderTimeout(status));
}
