use futures::StreamExt as _;
use scopeguard::defer;
use server_fn::ServerFnError;
use terrazzo::prelude::diagnostics::info;
use terrazzo::prelude::diagnostics::warn;
use tokio::task::spawn_local;

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
    });
}

async fn pipe_impl() -> Result<(), ServerFnError> {
    defer! {
        *super::stream_registration::stream_registrations() = None;
    }
    let mut pipe = crate::terminal::api::pipe().await?.into_inner();
    let mut parser = NdjsonBuffer::<PipeMessage>::default();
    while let Some(chunk) = pipe.next().await {
        let messages = parser.push_chunk(&chunk?);
        process_messages(messages)?;
    }
    Ok(())
}

fn process_messages(
    messages: Vec<Result<PipeMessage, serde_json::Error>>,
) -> Result<(), ServerFnError> {
    let mut lock = stream_registrations();
    for message in messages {
        let PipeMessage { terminal_id, chunk } = message?;
        let Some(stream_registrations) = &mut *lock else {
            return Err(ServerFnError::new("StreamRegistrations closed"));
        };
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
