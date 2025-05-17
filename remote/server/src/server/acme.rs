use std::time::Duration;

use instant_acme::Account;
use instant_acme::AccountCredentials;
use instant_acme::AuthorizationStatus;
use instant_acme::ChallengeType;
use instant_acme::Identifier;
use instant_acme::LetsEncrypt;
use instant_acme::NewAccount;
use instant_acme::NewOrder;
use instant_acme::OrderStatus;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use tokio::time::sleep;
use tracing::debug;
use tracing::info;

pub struct AcmeConfig {
    environment: LetsEncrypt,
    credentials: Option<AccountCredentials>,
    contact: String,
    domains: Vec<String>,
}

async fn get_certificate(config: AcmeConfig) -> Result<(), AcmeError> {
    let (account, credentials) = if let Some(credentials) = config.credentials {
        let a = Account::from_credentials(credentials).await?;
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
        .await?;
        (a, Some(c))
    };

    let identifiers = config
        .domains
        .iter()
        .map(|ident| Identifier::Dns(ident.to_owned()))
        .collect::<Vec<_>>();
    let mut order = account
        .new_order(&NewOrder {
            identifiers: identifiers.as_slice(),
        })
        .await?;

    let state = order.state();
    debug!("Order state: {:?}", state);
    debug_assert!(matches!(state.status, OrderStatus::Pending));

    let authorizations = order.authorizations().await?;
    info!("Order has {} authorizations", authorizations.len());

    for authorization in authorizations {
        match authorization.status {
            AuthorizationStatus::Pending => {}
            AuthorizationStatus::Valid => continue,
            AuthorizationStatus::Invalid => todo!(),
            AuthorizationStatus::Revoked => todo!(),
            AuthorizationStatus::Expired => todo!(),
        }

        // We'll use the DNS challenges for this example, but you could
        // pick something else to use here.

        let challenge = authorization
            .challenges
            .into_iter()
            .find(|c| c.r#type == ChallengeType::Http01)
            .ok_or(AcmeError::Http01ChallengeMissing)?;

        // TODO: Serve teh http01 challenge

        let () = order.set_challenge_ready(&challenge.url).await?;
    }

    // Exponentially back off until the order becomes ready or invalid.

    let status = order.refresh(5, Duration::from_millis(250)).await?;
    if status != OrderStatus::Ready {
        return Err(anyhow::anyhow!("unexpected order status: {status:?}"));
    }

    // Finalize the order and print certificate chain, private key and account credentials.

    let private_key_pem = order.finalize().await?;
    let cert_chain_pem = loop {
        match order.certificate().await? {
            Some(cert_chain_pem) => break cert_chain_pem,
            None => sleep(Duration::from_secs(1)).await,
        }
    };

    info!("certificate chain:\n\n{cert_chain_pem}");
    info!("private key:\n\n{private_key_pem}");
    Ok(())
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum AcmeError {
    #[error("[{}] {error}", self.name())]
    Acme {
        #[from]
        error: instant_acme::Error,
    },

    #[error("[{n}] Challenge for '{c:?}' not found", c = ChallengeType::Http01, n = self.name())]
    Http01ChallengeMissing,
}
