use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;

use futures::channel::mpsc;
use futures::channel::oneshot;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use server_fn::ServerFnError;

use self::pipe::ensure_pipe;
use crate::api::shared::terminal_schema::RegisterTerminalMode;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::terminal::streams::client::stream_registration::StreamRegistration;

mod pipe;
mod stream_registration;

pub async fn stream(
    mode: RegisterTerminalMode,
    terminal_def: TerminalDef,
) -> Result<StreamRegistration, StreamError> {
    let terminal_id = terminal_def.address.id.clone();
    let (tx, rx) = mpsc::unbounded();
    static STREAM_REGISTRATION_IDX: AtomicUsize = AtomicUsize::new(1);
    let idx = STREAM_REGISTRATION_IDX.fetch_add(1, SeqCst);
    let stream_registration = StreamRegistration {
        terminal_id: terminal_id.clone(),
        rx,
        idx,
    };
    ensure_pipe(|stream_registrations| {
        stream_registrations.map.insert(terminal_id, (idx, tx));
    })
    .await
    .map_err(StreamError::PipeError)?;
    let () = crate::terminal::api::add_stream(mode, terminal_def)
        .await
        .map_err(StreamError::AddStreamError)?;
    Ok(stream_registration)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum StreamError {
    #[error("[{n}] {0}", n = self.name())]
    AddStreamError(ServerFnError),

    #[error("[{n}] {0}", n = self.name())]
    PipeError(oneshot::Canceled),
}
