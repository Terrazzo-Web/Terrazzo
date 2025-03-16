use std::time::Duration;

use humantime::format_duration;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use prost_types::DurationError;
use tracing::info;
use trz_gateway_common::protos::terrazzo::remote::health::Ping;
use trz_gateway_common::protos::terrazzo::remote::health::Pong;
use trz_gateway_common::protos::terrazzo::remote::health::health_service_server::HealthService;

/// Implements [HealthService].
pub struct HealthServiceImpl;

#[tonic::async_trait]
impl HealthService for HealthServiceImpl {
    async fn ping_pong(
        &self,
        request: tonic::Request<Ping>,
    ) -> Result<tonic::Response<Pong>, tonic::Status> {
        let Ping {
            connection_id,
            delay,
        } = request.get_ref();

        let delay = if let Some(delay) = &delay {
            Some(Duration::try_from(*delay).map_err(PingError::from)?)
        } else {
            None
        };

        if let Some(delay) = &delay {
            let delay = format_duration(*delay);
            info!(connection_id, %delay, "Received ping");
        } else {
            info!(connection_id, "Received ping");
        }

        if let Some(delay) = delay {
            tokio::time::sleep(delay).await;
        }

        info!(connection_id, "Return pong");
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
