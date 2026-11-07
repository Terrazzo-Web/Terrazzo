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
        let entry = super::checkpoint::entry(&request.terminal, &request.attachment)?;
        let rows = u16::try_from(request.size.rows)
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| Status::invalid_argument("InvalidSize"))?;
        let cols = u16::try_from(request.size.cols)
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| Status::invalid_argument("InvalidSize"))?;
        drop(entry);
        let token = request
            .attachment
            .token
            .parse()
            .map_err(|_| Status::invalid_argument("InvalidToken"))?;
        processes::resize::resize(
            &request.terminal.id,
            rows.into(),
            cols.into(),
            token,
            &request.attachment.incarnation,
        )
        .await
        .map_err(|e| Status::internal(e.to_string()))
    }
);
