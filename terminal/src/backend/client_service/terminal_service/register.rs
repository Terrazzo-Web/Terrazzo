use std::sync::Arc;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use scopeguard::defer;
use terrazzo::http::StatusCode;
use terrazzo_pty::OpenProcessError;
use terrazzo_pty::ProcessIO;
use tonic::body::Body as BoxBody;
use tonic::client::GrpcService;
use tonic::codegen::Bytes;
use tonic::codegen::StdError;
use tonic::transport::Body;
use tracing::Instrument as _;
use tracing::info;
use tracing::info_span;
use trz_gateway_common::http_error::IsHttpError;
use trz_gateway_common::id::ClientName;

use crate::api::shared::terminal_schema::RegisterTerminalMode;
use crate::api::shared::terminal_schema::STREAMING_WINDOW_SIZE;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::backend::Server;
use crate::backend::client_service::routing::DistributedCallback;
use crate::backend::client_service::routing::DistributedCallbackError;
use crate::backend::protos::terrazzo::shared::ClientAddress;
use crate::backend::protos::terrazzo::terminal::RegisterTerminalRequest;
use crate::backend::protos::terrazzo::terminal::terminal_service_client::TerminalServiceClient;
use crate::backend::throttling_stream::ThrottleProcessOutput;
use crate::processes;
use crate::processes::io::HybridReader;

pub async fn register(
    my_client_name: Option<ClientName>,
    server: &Arc<Server>,
    mut request: RegisterTerminalRequest,
) -> Result<HybridReader, RegisterStreamError> {
    let terminal_def = request.def.get_or_insert_default();
    let client_address = terminal_def.client_address().to_vec();
    async move {
        info!("Start");
        defer!(info!("Done"));
        let stream =
            RegisterCallback::process(server, &client_address, (my_client_name, request)).await;
        let stream = stream.map_err(|error| error.map_local(Box::new))?;
        Ok(stream)
    }
    .instrument(info_span!("Register"))
    .await
}

struct RegisterCallback;

impl DistributedCallback for RegisterCallback {
    type Request = (Option<ClientName>, RegisterTerminalRequest);
    type Response = HybridReader;
    type LocalError = RegisterStreamError;
    type RemoteError = tonic::Status;

    async fn local(
        server: Option<&Arc<Server>>,
        (my_client_name, request): (Option<ClientName>, RegisterTerminalRequest),
    ) -> Result<HybridReader, RegisterStreamError> {
        let server =
            server.ok_or_else(|| RegisterStreamError::Grpc(tonic::Status::internal("server")))?;
        let mode = request.mode().try_into()?;
        let def = request
            .def
            .ok_or_else(|| tonic::Status::invalid_argument("def"))?;
        let def = TerminalDef::from(def);
        let terminal_id = def.address.id.clone();
        let stream = processes::stream::open_stream(
            server,
            def,
            mode == RegisterTerminalMode::Create,
            |_| async move {
                match mode {
                    RegisterTerminalMode::Create => {
                        let terminal_shell = server
                            .config()
                            .server
                            .with(|server_config| server_config.terminal_shell.clone());
                        ProcessIO::open(
                            my_client_name.map(|s| s.to_string()),
                            STREAMING_WINDOW_SIZE,
                            terminal_shell,
                        )
                        .await
                    }
                    RegisterTerminalMode::Reopen => Err(OpenProcessError::NotFound),
                }
            },
        )
        .await;
        let stream = stream.map_err(|error| tonic::Status::internal(error.to_string()))?;
        let stream = ThrottleProcessOutput::new(terminal_id, stream);
        return Ok(HybridReader::Local(stream));
    }

    async fn remote<T>(
        channel: T,
        client_address: &[impl AsRef<str>],
        (_, mut request): (Option<ClientName>, RegisterTerminalRequest),
    ) -> Result<HybridReader, tonic::Status>
    where
        T: GrpcService<BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        let def = request.def.as_mut();
        let def = def.ok_or_else(|| tonic::Status::invalid_argument("def"))?;
        let address = def.address.get_or_insert_default();
        address.via = Some(ClientAddress::of(client_address));
        let mut client = TerminalServiceClient::new(channel);
        let stream = client.register(request).await?.into_inner();
        Ok(HybridReader::Remote(Box::new(stream)))
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum RegisterStreamError {
    #[error("[{n}] {0}", n = self.name())]
    RegisterStreamError(#[from] DistributedCallbackError<Box<RegisterStreamError>, tonic::Status>),

    #[error("[{n}] {0}", n = self.name())]
    Grpc(#[from] tonic::Status),
}

impl IsHttpError for RegisterStreamError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::RegisterStreamError(error) => error.status_code(),
            Self::Grpc(error) => error.status_code(),
        }
    }
}

impl From<RegisterStreamError> for tonic::Status {
    fn from(error: RegisterStreamError) -> Self {
        match error {
            RegisterStreamError::RegisterStreamError(error) => error.into(),
            RegisterStreamError::Grpc(status) => status,
        }
    }
}

impl From<Box<RegisterStreamError>> for tonic::Status {
    fn from(error: Box<RegisterStreamError>) -> Self {
        tonic::Status::from(*error)
    }
}
