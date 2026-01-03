//! Ephemeral code to authenticate certifiate generation.

use std::sync::Mutex;
use std::time::Duration;

use futures::FutureExt as _;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use tokio::sync::oneshot;
use tracing::Instrument as _;
use trz_gateway_common::consts::HEALTH_CHECK_PERIOD;
use trz_gateway_common::consts::HEALTH_CHECK_TIMEOUT;
use trz_gateway_common::declare_identifier;
use uuid::Uuid;

const AUTH_CODE_UPDATE_PERIOD: Duration = Duration::from_millis(
    (HEALTH_CHECK_PERIOD.as_millis() + HEALTH_CHECK_TIMEOUT.as_millis()) as u64,
);

declare_identifier!(AuthCode);

impl AuthCode {
    /// The [AuthCode] that is currently valid.
    pub fn current() -> Self {
        let mut lock = CURRENT_CODE.lock().unwrap();
        if let Some(current_code) = &*lock {
            return current_code.current.clone();
        }

        let (tx, rx) = oneshot::channel();
        let current = AuthCode::new();
        let current_code = CurrentCode {
            periodic_updater: tx,
            previous: current.clone(),
            current: current.clone(),
        };
        *lock = Some(current_code);
        drop(lock);
        start_periodic_updates(rx);
        return current;
    }

    /// Checks if this [AuthCode] matches the [current](Self::current) one.
    ///
    /// The previous auth code is also valid
    pub fn is_valid(&self) -> bool {
        let lock = CURRENT_CODE.lock().unwrap();
        let Some(current_code) = &*lock else {
            return false;
        };
        return *self == current_code.current || *self == current_code.previous;
    }

    /// Stop periodic [AuthCode] updates.
    ///
    /// Calls to [current](Self::current) automatically kicks off a periodic task to
    /// rotate a random [AuthCode]. This function stops it.
    pub fn stop_periodic_updates() -> Result<(), StopPeriodicUpdatesError> {
        CURRENT_CODE
            .lock()
            .unwrap()
            .take()
            .ok_or(StopPeriodicUpdatesError::NotRunning)?
            .periodic_updater
            .send(())
            .map_err(|()| StopPeriodicUpdatesError::SignalFailed)
    }

    fn new() -> Self {
        Self::from(Uuid::new_v4().to_string())
    }
}

/// Errors thrown by [stop_periodic_updates](AuthCode::stop_periodic_updates).
#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum StopPeriodicUpdatesError {
    #[error("[{n}] Periodic {AUTH_CODE} updates are not scheduled", n = self.name() )]
    NotRunning,

    #[error("[{n}] Failed to send signal to stop periodic {AUTH_CODE} updates", n = self.name() )]
    SignalFailed,
}

static CURRENT_CODE: Mutex<Option<CurrentCode>> = Mutex::new(None);

struct CurrentCode {
    periodic_updater: oneshot::Sender<()>,
    previous: AuthCode,
    current: AuthCode,
}

impl CurrentCode {
    fn renew(&mut self) {
        self.previous = std::mem::replace(&mut self.current, AuthCode::new())
    }
}

fn start_periodic_updates(rx: oneshot::Receiver<()>) {
    tokio::spawn(
        async {
            let rx = rx.shared();
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(AUTH_CODE_UPDATE_PERIOD.max(Duration::from_secs(60))) => {}
                    _ = rx.clone() => { break; }
                }

                let mut lock = CURRENT_CODE.lock().unwrap();
                let Some(current_code) = &mut *lock else {
                    return;
                };
                current_code.renew();
            }
        }
        .in_current_span(),
    );
}

#[cfg(test)]
mod tests {
    use tokio::sync::Mutex;

    use super::AuthCode;
    use super::StopPeriodicUpdatesError;

    /// By default, Rust tests run in parallel
    static LOCK: Mutex<()> = Mutex::const_new(());

    #[tokio::test]
    async fn current() {
        let _lock = LOCK.lock().await;

        let auth_code = AuthCode::current();
        assert!(auth_code.is_valid());

        let () = AuthCode::stop_periodic_updates().unwrap();

        let auth_code2 = AuthCode::current();
        assert!(!auth_code.is_valid());
        assert!(auth_code2.is_valid());
        assert_ne!(auth_code, auth_code2);

        let () = AuthCode::stop_periodic_updates().unwrap();
    }

    #[tokio::test]
    async fn not_running() {
        let _lock = LOCK.lock().await;
        let error = AuthCode::stop_periodic_updates().unwrap_err();
        assert!(matches!(error, StopPeriodicUpdatesError::NotRunning));
        assert_eq!(
            "[NotRunning] Periodic AuthCode updates are not scheduled",
            error.to_string()
        );
    }
}
