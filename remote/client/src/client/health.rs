use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use humantime::format_duration;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use prost_types::DurationError;
use tokio::sync::oneshot;
use tokio::time::error::Elapsed;
use tracing::Instrument as _;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::info_span;
use tracing::warn;
use trz_gateway_common::consts::HEALTH_CHECK_PERIOD;
use trz_gateway_common::consts::HEALTH_CHECK_TIMEOUT;
use trz_gateway_common::protos::terrazzo::remote::health::Ping;
use trz_gateway_common::protos::terrazzo::remote::health::Pong;
use trz_gateway_common::protos::terrazzo::remote::health::health_service_server::HealthService;

use super::AuthCode;

/// Implements [HealthService].
pub struct HealthServiceImpl {
    current_auth_code: Arc<Mutex<AuthCode>>,
    health_report: Arc<Mutex<HealthReport>>,
}

struct HealthReport {
    report_ping: Option<oneshot::Sender<()>>,
    on_unhealthy: Option<oneshot::Sender<()>>,
}

impl HealthServiceImpl {
    pub fn new(current_auth_code: Arc<Mutex<AuthCode>>, on_unhealthy: oneshot::Sender<()>) -> Self {
        let health_service = Self {
            current_auth_code,
            health_report: Arc::new(Mutex::new(HealthReport {
                report_ping: None,
                on_unhealthy: Some(on_unhealthy),
            })),
        };
        health_service.schedule_timeout();
        return health_service;
    }

    fn schedule_timeout(&self) {
        let health_report = Arc::downgrade(&self.health_report);
        let (next_ping_tx, next_ping_rx) = oneshot::channel();
        {
            let mut lock = self.health_report.lock().expect("health_report");
            if let Some(report_ping) = lock.report_ping.replace(next_ping_tx) {
                let _ = report_ping.send(());
            }
        }
        let task = async move {
            match tokio::time::timeout(HEALTH_CHECK_PERIOD + HEALTH_CHECK_TIMEOUT, next_ping_rx)
                .await
            {
                Err(Elapsed { .. }) => {}
                Ok(Ok(())) => {
                    debug!("The ping was received");
                    return;
                }
                Ok(Err(oneshot::error::RecvError { .. })) => {
                    debug!("The health report was dropped");
                    return;
                }
            }
            let Some(health_report) = health_report.upgrade() else {
                warn!("Health report timed out without being canceled when client was dropped");
                return;
            };
            let mut lock = health_report.lock().expect("health_report");
            if let Some(on_unhealthy) = lock.on_unhealthy.take() {
                warn!(
                    "No ping was received after PERIOD={} + TIMEOUT={}",
                    humantime::format_duration(HEALTH_CHECK_PERIOD),
                    humantime::format_duration(HEALTH_CHECK_TIMEOUT)
                );
                match on_unhealthy.send(()) {
                    Ok(()) => debug!("Notified the connection was unhealthy"),
                    Err(()) => error!("Failed to notify the connection was unhealthy"),
                };
            }
        };
        tokio::spawn(task.instrument(info_span!("Health report")));
    }
}

#[tonic::async_trait]
impl HealthService for HealthServiceImpl {
    async fn ping_pong(
        &self,
        request: tonic::Request<Ping>,
    ) -> Result<tonic::Response<Pong>, tonic::Status> {
        self.schedule_timeout();
        let Ping {
            connection_id,
            delay,
            auth_code,
        } = request.into_inner();

        if !auth_code.is_empty() {
            let auth_code = auth_code.into();
            if cfg!(debug_assertions) {
                let current_auth_code = self.current_auth_code.lock().unwrap().clone();
                debug!(
                    changed = (current_auth_code != auth_code),
                    "Got a new AuthCode"
                )
            }
            *self.current_auth_code.lock().unwrap() = auth_code;
        }

        if let Some(delay) = delay {
            let delay = Duration::try_from(delay).map_err(PingError::from)?;
            let delay_printed = format_duration(delay);
            info!(connection_id, delay = %delay_printed, "Received ping");
            tokio::time::sleep(delay).await;
        } else {
            info!(connection_id, "Received ping");
        };

        debug!(connection_id, "Return pong");
        Ok(tonic::Response::new(Pong {}))
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum PingError {
    #[error("[{n}] {0}", n = self.name())]
    InvalidDelay(#[from] DurationError),
}

impl From<PingError> for tonic::Status {
    fn from(error: PingError) -> Self {
        Self::new(error.to_status_code(), error.to_string())
    }
}

impl PingError {
    fn to_status_code(&self) -> tonic::Code {
        match self {
            PingError::InvalidDelay { .. } => tonic::Code::InvalidArgument,
        }
    }
}
