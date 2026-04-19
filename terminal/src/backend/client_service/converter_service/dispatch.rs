use nameth::NamedEnumValues as _;
use nameth::nameth;
use tonic::Code;
use tonic::Status;

use super::callback::ConverterCallback;
use super::callback::ConverterLocalError;
use super::response::HybridResponseStream;
use crate::backend::client_service::remote_fn_service::remote_fn_server;
use crate::backend::client_service::routing::DistributedCallback as _;
use crate::backend::client_service::routing::DistributedCallbackError;
use crate::backend::protos::terrazzo::converter::ConversionsRequest;

pub async fn conversions_dispatch(
    request: ConversionsRequest,
) -> Result<HybridResponseStream, ConversionsError> {
    let server = remote_fn_server().ok();
    let client_address = request
        .address
        .as_ref()
        .map(|address| address.via.clone())
        .unwrap_or_default();
    ConverterCallback::process(server.as_ref(), &client_address, request)
        .await
        .map_err(ConversionsError::Error)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ConversionsError {
    #[error("[{n}] {0}", n = self.name())]
    Error(DistributedCallbackError<ConverterLocalError, Status>),
}

impl From<ConversionsError> for Status {
    fn from(mut error: ConversionsError) -> Self {
        let code = match &mut error {
            ConversionsError::Error(DistributedCallbackError::RemoteError(error)) => {
                return std::mem::replace(error, Status::ok(""));
            }
            ConversionsError::Error(DistributedCallbackError::LocalError { .. })
            | ConversionsError::Error(DistributedCallbackError::ServerNotSet) => Code::Internal,
            ConversionsError::Error(DistributedCallbackError::RemoteClientNotFound { .. }) => {
                Code::NotFound
            }
        };
        Status::new(code, error.to_string())
    }
}
