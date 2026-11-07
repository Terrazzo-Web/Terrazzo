use server_fn::ServerFnError;
use tonic::Status;

use crate::api::shared::terminal_schema::AppliedCursor;
use crate::api::shared::terminal_schema::Attachment;
use crate::api::shared::terminal_schema::TerminalAddress;
use crate::backend::client_service::remote_fn_service;

pub async fn ack(
    terminal: TerminalAddress,
    attachment: Attachment,
    cursor: AppliedCursor,
) -> Result<(), ServerFnError> {
    Ok(ACK_FN
        .call(terminal.via.clone(), (terminal, attachment, cursor))
        .await?)
}

remote_fn_service::unary::declare_remote_fn!(
    ACK_FN,
    "terminal.ack",
    (TerminalAddress, Attachment, AppliedCursor),
    (),
    |_server, (terminal, attachment, cursor)| async move {
        let entry = super::checkpoint::entry(&terminal, &attachment)?;
        let token = attachment
            .token
            .parse()
            .map_err(|_| Status::invalid_argument("InvalidToken"))?;
        entry
            .journal
            .validate_end(
                token,
                cursor
                    .event
                    .parse()
                    .map_err(|_| Status::invalid_argument("InvalidCursor"))?,
                cursor
                    .end
                    .parse()
                    .map_err(|_| Status::invalid_argument("InvalidCursor"))?,
            )
            .map_err(Status::invalid_argument)?;
        entry
            .journal
            .ack(
                token,
                cursor
                    .event
                    .parse()
                    .map_err(|_| Status::invalid_argument("InvalidCursor"))?,
            )
            .map_err(Status::invalid_argument)?;
        crate::backend::throttling_stream::ack(
            &terminal.id,
            token,
            cursor
                .event
                .parse()
                .map_err(|_| Status::invalid_argument("InvalidCursor"))?,
            cursor
                .end
                .parse()
                .map_err(|_| Status::invalid_argument("InvalidCursor"))?,
        );
        Ok::<_, Status>(())
    }
);
