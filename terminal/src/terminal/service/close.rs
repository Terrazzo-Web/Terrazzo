use server_fn::ServerFnError;
use tonic::Status;

use crate::api::shared::terminal_schema::TerminalAddress;
use crate::backend::client_service::remote_fn_service;
use crate::processes;
use crate::terminal_id::TerminalId;

pub async fn close(terminal: TerminalAddress) -> Result<(), ServerFnError> {
    Ok(CLOSE_FN.call(terminal.via, terminal.id).await?)
}

remote_fn_service::unary::declare_remote_fn!(
    CLOSE_FN,
    "terminal.close",
    TerminalId,
    (),
    |_server, terminal_id: TerminalId| async move {
        processes::close::close(&terminal_id).map_err(|e| Status::not_found(e.to_string()))
    }
);
