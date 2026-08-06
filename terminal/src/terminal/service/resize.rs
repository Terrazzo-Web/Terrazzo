use server_fn::ServerFnError;
use tonic::Status;

use crate::api::shared::terminal_schema::ResizeRequest;
use crate::backend::client_service::remote_fn_service;
use crate::processes;

pub async fn resize(request: ResizeRequest) -> Result<(), ServerFnError> {
    Ok(RESIZE_FN
        .call(request.terminal.via.clone(), request)
        .await?)
}

remote_fn_service::unary::declare_remote_fn!(
    RESIZE_FN,
    "terminal.resize",
    ResizeRequest,
    (),
    |_server, request: ResizeRequest| async move {
        processes::resize::resize(
            &request.terminal.id,
            request.size.rows,
            request.size.cols,
            request.force,
        )
        .await
        .map_err(|e| Status::internal(e.to_string()))
    }
);
