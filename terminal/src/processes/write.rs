use nameth::NamedEnumValues as _;
use nameth::nameth;
use terrazzo::http::StatusCode;
use tokio::io::AsyncWriteExt as _;
use tracing::trace;
use trz_gateway_common::http_error::IsHttpError;

use super::get_processes;
use crate::terminal_id::TerminalId;

pub async fn write(
    terminal_id: &TerminalId,
    data: &[u8],
    attachment: Option<(&str, u64)>,
) -> Result<(), WriteError> {
    trace!(bytes = data.len(), "Writing terminal input");
    let processes = get_processes();
    let entry = {
        let Some(entry) = processes.get(terminal_id) else {
            return Err(WriteError::TerminalNotFound {
                terminal_id: terminal_id.to_owned(),
            });
        };
        entry.value().1.clone()
    };
    let mut input = entry.input().await;
    if let Some((incarnation, token)) = attachment {
        if entry.incarnation != incarnation {
            return Err(WriteError::StaleAttachment);
        }
        entry
            .journal
            .validate(token)
            .map_err(|_| WriteError::StaleAttachment)?;
    }
    return input.write_all(data).await.map_err(WriteError::Write);
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum WriteError {
    #[error("StaleAttachment")]
    StaleAttachment,
    #[error("[{n}] Terminal not found {terminal_id}", n = self.name())]
    TerminalNotFound { terminal_id: TerminalId },

    #[error("[{n}] Failed to write: {0}", n = self.name())]
    Write(std::io::Error),
}

impl IsHttpError for WriteError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::StaleAttachment => StatusCode::CONFLICT,
            Self::TerminalNotFound { .. } => StatusCode::NOT_FOUND,
            Self::Write { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
