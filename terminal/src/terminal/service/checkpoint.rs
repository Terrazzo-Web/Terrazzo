use std::sync::Arc;

use server_fn::ServerFnError;
use terrazzo_pty::lease::ProcessIoEntry;
use terrazzo_pty::replay::Checkpoint;
use tonic::Status;

use crate::api::shared::terminal_schema::Attachment;
use crate::api::shared::terminal_schema::TerminalAddress;
use crate::api::shared::terminal_schema::TerminalCheckpoint;
use crate::backend::client_service::remote_fn_service;

pub fn entry(
    terminal: &TerminalAddress,
    attachment: &Attachment,
) -> Result<Arc<ProcessIoEntry>, Status> {
    let entry = crate::processes::get_processes()
        .get(&terminal.id)
        .map(|entry| entry.1.clone())
        .ok_or_else(|| Status::not_found("ProcessGone"))?;
    if entry.incarnation != attachment.incarnation {
        return Err(Status::failed_precondition("FreshRestoreRequired"));
    }
    entry
        .journal
        .validate(
            attachment
                .token
                .parse()
                .map_err(|_| Status::invalid_argument("InvalidToken"))?,
        )
        .map_err(Status::failed_precondition)?;
    Ok(entry)
}

pub async fn put(
    terminal: TerminalAddress,
    checkpoint: TerminalCheckpoint,
) -> Result<bool, ServerFnError> {
    Ok(CHECKPOINT_FN
        .call(terminal.via.clone(), (terminal, checkpoint))
        .await?)
}

remote_fn_service::unary::declare_remote_fn!(
    CHECKPOINT_FN,
    "terminal.checkpoint",
    (TerminalAddress, TerminalCheckpoint),
    bool,
    |_server, (terminal, checkpoint): (TerminalAddress, TerminalCheckpoint)| async move {
        if checkpoint.version != 1 {
            return Err(Status::failed_precondition("UnsupportedFormat"));
        }
        let entry = entry(&terminal, &checkpoint.attachment)?;
        let token = checkpoint
            .attachment
            .token
            .parse()
            .map_err(|_| Status::invalid_argument("InvalidToken"))?;
        let cursor = checkpoint
            .cursor
            .event
            .parse()
            .map_err(|_| Status::invalid_argument("InvalidCursor"))?;
        entry
            .journal
            .ack(token, cursor)
            .map_err(Status::invalid_argument)?;
        entry
            .journal
            .commit(
                token,
                Checkpoint {
                    event: cursor,
                    end: checkpoint
                        .cursor
                        .end
                        .parse()
                        .map_err(|_| Status::invalid_argument("InvalidCursor"))?,
                    rows: u16::try_from(checkpoint.size.rows)
                        .map_err(|_| Status::invalid_argument("InvalidSize"))?,
                    cols: u16::try_from(checkpoint.size.cols)
                        .map_err(|_| Status::invalid_argument("InvalidSize"))?,
                    payload: checkpoint.payload.into_bytes().into(),
                },
            )
            .map_err(Status::invalid_argument)
    }
);
