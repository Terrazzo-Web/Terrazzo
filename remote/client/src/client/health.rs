use nameth::nameth;
use nameth::NamedEnumValues as _;
use prost_types::DurationError;
use tracing::info;
use trz_gateway_common::protos::terrazzo::remote::health::health_service_server::HealthService;
use trz_gateway_common::protos::terrazzo::remote::health::Ping;
use trz_gateway_common::protos::terrazzo::remote::health::Pong;

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
        info!(connection_id, ?delay, "Received ping");
        if let Some(delay) = delay {
            let delay = (*delay).try_into().map_err(PingError::from)?;
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
