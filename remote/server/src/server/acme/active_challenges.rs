use std::collections::HashMap;
use std::future::ready;
use std::sync::Arc;

use axum::Router;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::get;
use instant_acme::KeyAuthorization;
use tracing::warn;

#[derive(Clone, Default)]
pub struct ActiveChallenges(Arc<std::sync::RwLock<HashMap<String, KeyAuthorization>>>);

impl ActiveChallenges {
    pub fn route(&self) -> Router {
        let this = self.to_owned();
        Router::new().route(
            "/.well-known/acme-challenge/{token}",
            get(move |Path(token): Path<String>| {
                ready(match this.0.read().unwrap().get(&token) {
                    Some(key_authorization) => {
                        (StatusCode::OK, key_authorization.as_str().to_string())
                    }
                    None => (StatusCode::NOT_FOUND, String::default()),
                })
            }),
        )
    }

    pub fn add(&self, token: &str, key_authorization: KeyAuthorization) -> ActiveChallenge {
        let old = {
            let mut active_challenges = self.0.write().unwrap();
            active_challenges.insert(token.to_owned(), key_authorization)
        };
        if old.is_some() {
            warn!("Duplicate token: {token}");
        }
        ActiveChallenge {
            active_challenges: self.to_owned(),
            token: token.to_owned(),
        }
    }
}

pub struct ActiveChallenge {
    active_challenges: ActiveChallenges,
    token: String,
}

impl Drop for ActiveChallenge {
    fn drop(&mut self) {
        let mut active_challenges = self.active_challenges.0.write().unwrap();
        active_challenges.remove(&self.token);
    }
}
