use std::ops::ControlFlow;
use std::time::Duration;

use instant_acme::Account;
use instant_acme::AccountCredentials;
use instant_acme::AuthorizationStatus;
use instant_acme::ChallengeType;
use instant_acme::Identifier;
use instant_acme::NewAccount;
use instant_acme::NewOrder;
use instant_acme::Order;
use instant_acme::OrderStatus;
use rcgen::CertificateParams;
use rcgen::DistinguishedName;
use rcgen::KeyPair;
use tokio::time::sleep;
use tracing::debug;
use tracing::info;
use trz_gateway_common::certificate_info::CertificateInfo;

use super::AcmeConfig;
use super::AcmeError;
use super::active_challenges::ActiveChallenges;
use crate::server::acme::clone_account_credentials;

pub struct GetAcmeCertificateResult {
    pub certificate: CertificateInfo<String>,
    pub credentials: Option<AccountCredentials>,
}

impl AcmeConfig {
    pub(super) async fn get_certificate(
        &self,
        active_challenges: &ActiveChallenges,
    ) -> Result<GetAcmeCertificateResult, AcmeError> {
        debug!("Get or create Let's Encrypt account");
        let (account, credentials) = if let Some(credentials) = self.credentials.as_ref() {
            let account = Account::from_credentials(clone_account_credentials(credentials))
                .await
                .map_err(AcmeError::FromCredentials)?;
            (account, None)
        } else {
            let (account, credentials) = Account::create(
                &NewAccount {
                    contact: &[&self.contact],
                    terms_of_service_agreed: true,
                    only_return_existing: false,
                },
                self.environment.url(),
                None,
            )
            .await
            .map_err(AcmeError::CreateAccount)?;
            (account, Some(credentials))
        };

        info!("Got account ID = {}", account.id());

        let identifier = Identifier::Dns(self.domain.clone());
        let mut order = account
            .new_order(&NewOrder {
                identifiers: &[identifier],
            })
            .await
            .map_err(AcmeError::NewOrder)?;

        let state = order.state();
        info!("Order created in state: {:?}", state);

        let authorizations = order
            .authorizations()
            .await
            .map_err(AcmeError::Authorizations)?;
        info!("Order has {} authorizations", authorizations.len());
        let mut challenges = Vec::with_capacity(authorizations.len());

        let mut registrations = vec![];
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

            let token = challenge.token.as_str();
            let key_authorization = order.key_authorization(challenge);
            registrations.push(active_challenges.add(token, key_authorization));

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

        let status = poll_order(&mut order, 5, Duration::from_millis(250)).await?;
        info!("Order status: {:?}", status);

        // If the order is ready, we can provision the certificate.
        // Use the rcgen library to create a Certificate Signing Request.

        let mut params = CertificateParams::new(vec![self.domain.clone()])?;
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

        drop(registrations);

        Ok(GetAcmeCertificateResult {
            certificate: CertificateInfo {
                certificate,
                private_key: private_key.serialize_pem(),
            },
            credentials,
        })
    }
}

async fn poll_order(
    order: &mut Order,
    tries: i32,
    delay: Duration,
) -> Result<OrderStatus, AcmeError> {
    let poll_task = poll(
        |(order, _last_status)| async {
            let state = match order.refresh().await.map_err(AcmeError::Refresh) {
                Ok(state) => state,
                Err(error) => return ControlFlow::Break(Err(error)),
            };
            let status = state.status;
            info!("Order is now in state: {status:?}");
            let result = match status {
                OrderStatus::Pending | OrderStatus::Processing => {
                    return ControlFlow::Continue((order, status));
                }
                OrderStatus::Ready | OrderStatus::Valid => Ok(status),
                OrderStatus::Invalid => Err(AcmeError::OrderFailed(status)),
            };
            return ControlFlow::Break(result);
        },
        (order, OrderStatus::Pending),
        tries,
        delay,
    );
    match poll_task.await {
        Ok(result) => result,
        Err((_, last_status)) => Err(AcmeError::OrderTimeout(last_status)),
    }
}

async fn poll<S, FR, R>(
    mut f: impl FnMut(S) -> FR,
    mut state: S,
    mut tries: i32,
    mut delay: Duration,
) -> Result<R, S>
where
    FR: Future<Output = ControlFlow<R, S>>,
{
    while tries > 0 {
        tokio::time::sleep(delay).await;
        match f(state).await {
            ControlFlow::Continue(new_state) => state = new_state,
            ControlFlow::Break(result) => return Ok(result),
        }
        delay *= 2;
        tries -= 1;
    }
    return Err(state);
}
