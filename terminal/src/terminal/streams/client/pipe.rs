use futures::StreamExt as _;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use scopeguard::defer;
use server_fn::ServerFnError;
use terrazzo::prelude::diagnostics::info;
use terrazzo::prelude::diagnostics::warn;
use wasm_bindgen_futures::spawn_local;

use super::stream_registration::StreamRegistrations;
use super::stream_registration::stream_registrations;
use crate::terminal::api::PipeMessage;
use crate::utils::ndjson::NdjsonBuffer;

pub fn ensure_pipe(f: impl FnOnce(&mut StreamRegistrations)) {
    {
        let mut lock = stream_registrations();
        if let Some(stream_registrations) = &mut *lock {
            f(stream_registrations);
            return;
        }
        let mut stream_registrations = StreamRegistrations::new();
        f(&mut stream_registrations);
        *lock = Some(stream_registrations)
    }

    spawn_local(async {
        match pipe_impl().await {
            Ok(()) => info!("Pipe closed"),
            Err(error) => warn!("Pipe failed: {error}"),
        }
    })
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum PipeError {
    #[error("[{n}] {0}", n = self.name())]
    PipeOpenError(ServerFnError),

    #[error("[{n}] {0}", n = self.name())]
    PipeMessagesError(ServerFnError),

    #[error("[{n}] Invalid 'PipeMessage': {0}", n = self.name())]
    PipeJsonError(serde_json::Error),

    #[error("[{n}] Stream registrations closed", n = self.name())]
    StreamRegistrationsClosed,
}

async fn pipe_impl() -> Result<(), PipeError> {
    defer! {
        *super::stream_registration::stream_registrations() = None;
    }
    let mut pipe = crate::terminal::api::pipe()
        .await
        .map_err(PipeError::PipeOpenError)?
        .into_inner();
    let mut parser = NdjsonBuffer::<PipeMessage>::default();
    while let Some(chunk) = pipe.next().await {
        let messages = parser.push_chunk(&chunk.map_err(PipeError::PipeMessagesError)?);
        process_messages(messages)?;
    }
    Ok(())
}

fn process_messages(
    messages: Vec<Result<PipeMessage, serde_json::Error>>,
) -> Result<(), PipeError> {
    let mut lock = stream_registrations();
    let Some(stream_registrations) = &mut *lock else {
        return Err(PipeError::StreamRegistrationsClosed);
    };
    for message in messages {
        let PipeMessage { terminal_id, chunk } = message.map_err(PipeError::PipeJsonError)?;
        if let Some(tx) = stream_registrations.get_mut(&terminal_id) {
            match tx.try_send(chunk) {
                Ok(()) => (),
                Err(error) => {
                    // TODO: to avoid this, the ack window should be smaller than the channel buffer size.
                    warn!(%terminal_id, "Failed to send: {error}");
                    stream_registrations.remove(&terminal_id);
                }
            }
        } else {
            warn!(%terminal_id, "Stream is not registered");
        }
    }
    Ok(())
}
