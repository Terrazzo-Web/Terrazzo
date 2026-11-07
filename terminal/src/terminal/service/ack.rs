use server_fn::ServerFnError;
use tonic::Status;

use crate::api::shared::terminal_schema::TerminalAddress;
use crate::backend::client_service::remote_fn_service;
use crate::terminal_id::TerminalId;

pub async fn ack(terminal: TerminalAddress, ack: usize) -> Result<(), ServerFnError> {
    Ok(ACK_FN
        .call(terminal.via.clone(), (terminal.id, ack))
        .await?)
}

remote_fn_service::unary::declare_remote_fn!(
    ACK_FN,
    "terminal.ack",
    (TerminalId, usize),
    (),
    |_server, (terminal_id, ack)| async move {
        crate::backend::throttling_stream::ack(&terminal_id, ack);
        Ok::<_, Status>(())
    }
);
