use std::sync::Arc;
use std::sync::LazyLock;

use axum_extra::extract::CookieJar;
use terrazzo::autoclone;
use terrazzo::axum::Json;
use terrazzo::axum::Router;
use terrazzo::axum::response::IntoResponse;
use terrazzo::axum::routing::post;
use terrazzo::http::HeaderMap;
use terrazzo::http::StatusCode;
use tokio::sync::Semaphore;
use tracing::Instrument as _;
use tracing::debug;
use tracing::info;
use tracing::info_span;
use tracing::warn;

use crate::backend::config::DynConfig;
use crate::backend::config::WatchConfig as _;

#[autoclone]
pub fn login_routes(config: &Arc<DynConfig>) -> Router {
    Router::new().route(
        "/login",
        post(|cookies, headers, password| {
            autoclone!(config);
            login(config, cookies, headers, password)
        }),
    )
}

#[autoclone]
async fn login(
    config: Arc<DynConfig>,
    cookies: CookieJar,
    headers: HeaderMap,
    Json(password): Json<Option<String>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let result = async move {
        let server = config.server.get();
        let auth_config = server.auth.clone();
        match (&server.password, &password) {
            (None, _) => debug!("Password not required"),
            (Some(_), None) => {
                debug!("Password not provided, checking token");
                let _ = auth_config.validate(&headers)?;
            }
            (Some(_), Some(password)) => {
                debug!("Password provided, verify password");
                let () =
                    run_password_verification(PASSWORD_VERIFICATION_LIMIT.clone(), move || {
                        autoclone!(password);
                        server.verify_password(&password)
                    })
                    .await
                    .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
                    .map_err(|error| (StatusCode::UNAUTHORIZED, error.to_string()))?;
            }
        }

        let token = auth_config
            .make_token()
            .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
        Ok((cookies.add(token), "OK"))
    }
    .instrument(info_span!("Login"))
    .await;
    return result
        .inspect(|(_cookies, result)| info!("{result}"))
        .inspect_err(|(status_code, error)| warn!("Failed: {status_code} {error}"));
}

// Match the global attempt-state serialization without parking blocking-pool threads
// on its mutex. Acquire before spawning, and retain admission after cancellation.
static PASSWORD_VERIFICATION_LIMIT: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(1)));

async fn run_password_verification<T: Send + 'static>(
    limit: Arc<Semaphore>,
    verify: impl FnOnce() -> T + Send + 'static,
) -> Result<T, tokio::task::JoinError> {
    let permit = limit
        .acquire_owned()
        .await
        .expect("verification semaphore closed");
    let span = tracing::Span::current();
    tokio::task::spawn_blocking(move || {
        let _span = span.enter();
        let result = verify();
        drop(permit);
        return result;
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::sync::Semaphore;
    use tokio::sync::oneshot;
    use tokio::time::timeout;

    use super::run_password_verification;

    // Current-thread runtime: the notification cannot arrive if verification
    // blocks the async worker. Releasing on sender drop also unblocks on panic.
    #[tokio::test]
    async fn verification_yields_and_retains_admission_after_cancellation() {
        let limit = Arc::new(Semaphore::new(1));
        let (started, running) = oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let first = tokio::spawn(run_password_verification(limit.clone(), move || {
            let _ = started.send(());
            let _ = wait.recv_timeout(Duration::from_secs(10));
        }));
        timeout(Duration::from_secs(5), running)
            .await
            .unwrap()
            .unwrap();
        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());
        assert_eq!(limit.available_permits(), 0);

        let (started, mut running) = oneshot::channel();
        let second = tokio::spawn(run_password_verification(limit.clone(), move || {
            let _ = started.send(());
            42
        }));
        tokio::task::yield_now().await;
        assert!(matches!(
            running.try_recv(),
            Err(oneshot::error::TryRecvError::Empty)
        ));
        release.send(()).unwrap();
        assert_eq!(
            timeout(Duration::from_secs(5), second)
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
            42
        );
        running.await.unwrap();
        assert_eq!(limit.available_permits(), 1);
    }

    #[tokio::test]
    async fn verification_failure_releases_admission() {
        let limit = Arc::new(Semaphore::new(1));
        let error = run_password_verification(limit.clone(), || panic!("verification failed"))
            .await
            .unwrap_err();
        assert!(error.is_panic());
        assert_eq!(limit.available_permits(), 1);
        assert_eq!(run_password_verification(limit, || 42).await.unwrap(), 42);
    }
}
