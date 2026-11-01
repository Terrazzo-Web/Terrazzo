use server_fn::ServerFnError;
use tonic::Status;

use crate::api::shared::terminal_schema::Attachment;
use crate::api::shared::terminal_schema::TerminalAddress;
use crate::api::shared::terminal_schema::TerminalInput;
use crate::backend::client_service::remote_fn_service;
use crate::processes;

pub async fn write(
    terminal: TerminalAddress,
    attachment: Attachment,
    data: Vec<TerminalInput>,
) -> Result<(), ServerFnError> {
    Ok(WRITE_FN
        .call(terminal.via.clone(), (terminal, attachment, data))
        .await?)
}

remote_fn_service::unary::declare_remote_fn!(
    WRITE_FN,
    "terminal.write",
    (TerminalAddress, Attachment, Vec<TerminalInput>),
    (),
    |_server, (terminal, attachment, data)| async move {
        let entry = super::checkpoint::entry(&terminal, &attachment)?;
        let token = attachment
            .token
            .parse()
            .map_err(|_| Status::invalid_argument("InvalidToken"))?;
        drop(entry);
        let data: Vec<u8> = data
            .into_iter()
            .flat_map(|input| match input {
                TerminalInput::Text(text) => text.into_bytes(),
                TerminalInput::Binary(bytes) => bytes,
            })
            .collect();
        processes::write::write(&terminal.id, &data, Some((&attachment.incarnation, token)))
            .await
            .map_err(|e| Status::internal(e.to_string()))
    }
);
