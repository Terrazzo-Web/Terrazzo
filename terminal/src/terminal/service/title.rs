use server_fn::ServerFnError;
use tonic::Status;

use crate::api::shared::terminal_schema::SetTitleRequest;
use crate::backend::client_service::remote_fn_service;
use crate::processes;

pub async fn set_title(request: SetTitleRequest) -> Result<(), ServerFnError> {
    Ok(SET_TITLE_FN
        .call(request.terminal.via.clone(), request)
        .await?)
}

remote_fn_service::unary::declare_remote_fn!(
    SET_TITLE_FN,
    "terminal.set_title",
    SetTitleRequest,
    (),
    |_server, request: SetTitleRequest| async move {
        processes::set_title::set_title(&request.terminal.id, request.title)
            .map_err(|e| Status::not_found(e.to_string()))
    }
);
