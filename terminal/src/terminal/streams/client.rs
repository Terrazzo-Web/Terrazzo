use futures::channel::mpsc;
use server_fn::ServerFnError;

use self::pipe::ensure_pipe;
use crate::api::shared::terminal_schema::RegisterTerminalMode;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::terminal::streams::client::stream_registration::StreamRegistration;

mod pipe;
mod stream_registration;

const STREAM_DISPATCH_BUFFER_SIZE: usize = 10;

pub async fn stream(
    mode: RegisterTerminalMode,
    terminal_def: TerminalDef,
) -> Result<StreamRegistration, ServerFnError> {
    let terminal_id = terminal_def.address.id.clone();
    let (tx, rx) = mpsc::channel(STREAM_DISPATCH_BUFFER_SIZE);
    let stream_registration = StreamRegistration {
        terminal_id: terminal_id.clone(),
        rx,
    };
    ensure_pipe(|stream_registrations| {
        stream_registrations.insert(terminal_id, tx);
    });
    let () = crate::terminal::api::add_stream(mode, terminal_def).await?;
    Ok(stream_registration)
}
