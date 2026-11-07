use server_fn::ServerFnError;
use tonic::Status;

use crate::api::shared::terminal_schema::TerminalAddress;
use crate::backend::client_service::remote_fn_service;
use crate::processes;

pub async fn write(terminal: TerminalAddress, data: String) -> Result<(), ServerFnError> {
    Ok(WRITE_FN
        .call(terminal.via.clone(), (terminal, data))
        .await?)
}

remote_fn_service::unary::declare_remote_fn!(
    WRITE_FN,
    "terminal.write",
    (TerminalAddress, String),
    (),
    |_server, (terminal, data)| async move {
        processes::write::write(&terminal.id, data.as_bytes())
            .await
            .map_err(|e| Status::internal(e.to_string()))
    }
);
