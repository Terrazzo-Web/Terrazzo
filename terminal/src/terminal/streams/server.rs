use std::sync::Mutex;

use futures::StreamExt;
use futures::channel::mpsc;
use server_fn::ServerFnError;
use server_fn::codec::TextStream;

use crate::api::shared::terminal_schema::RegisterTerminalMode;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::terminal::api::PipeMessage;
use crate::terminal_id::TerminalId;
use crate::utils::ndjson_utils::serialize_line;

static PIPE: Mutex<Option<mpsc::Sender<PipedStream>>> = Mutex::new(None);

struct PipedStream {
    terminal_id: TerminalId,
    stream: TextStream,
}

pub async fn pipe() -> TextStream {
    let (pipe_tx, pipe_rx) = mpsc::channel(1);
    *PIPE.lock().expect("pipe") = Some(pipe_tx);
    let pipe = pipe_rx.flat_map_unordered(
        None,
        |PipedStream {
             terminal_id,
             stream,
         }| {
            stream.into_inner().map(move |chunk| PipeMessage {
                terminal_id: terminal_id.clone(),
                chunk,
            })
        },
    );

    let pipe = pipe.map(|item| serialize_line(&item).map_err(ServerFnError::from));

    return TextStream::new(pipe);
}

pub async fn add_stream(
    mode: RegisterTerminalMode,
    terminal_def: TerminalDef,
) -> Result<(), ServerFnError> {
    let terminal_id = terminal_def.address.id.clone();
    let stream = crate::terminal::service::stream::stream(mode, terminal_def).await?;
    let mut lock = PIPE.lock().expect("PIPE");
    if let Some(pipe) = &mut *lock {
        let () = pipe.try_send(PipedStream {
            terminal_id,
            stream,
        })?;
        Ok(())
    } else {
        Err(ServerFnError::new("pipe was closed"))
    }
}
