use futures::channel::mpsc;
use futures::channel::oneshot;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use server_fn::ServerFnError;
use terrazzo::prelude::with_generation_id::WithGenerationId;

use self::pipe::ensure_pipe;
use crate::api::shared::terminal_schema::AttachRequest;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::terminal::streams::client::stream_registration::StreamRegistration;

mod pipe;
mod stream_registration;

pub async fn stream(
    request: AttachRequest,
    terminal_def: TerminalDef,
) -> Result<StreamRegistration, StreamError> {
    let terminal_id = terminal_def.address.id.clone();
    let (tx, rx) = mpsc::unbounded();
    let tx = WithGenerationId::from(tx);
    let registration = format!(
        "{}-{}-{}",
        web_sys::js_sys::Date::now(),
        web_sys::js_sys::Math::random(),
        tx.generation_id
    );
    let stream_registration = StreamRegistration {
        terminal_id: terminal_id.clone(),
        rx,
        generation_id: tx.generation_id,
        registration: registration.clone(),
    };
    ensure_pipe(|stream_registrations| {
        stream_registrations.map.insert(
            terminal_id,
            stream_registration::RegisteredStream {
                registration,
                sender: tx,
            },
        );
    })
    .await
    .map_err(StreamError::PipeError)?;
    let () = crate::terminal::api::add_stream(
        request,
        terminal_def,
        stream_registration.registration.clone(),
    )
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
