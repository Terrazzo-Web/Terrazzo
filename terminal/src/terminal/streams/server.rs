use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;

use futures::SinkExt as _;
use futures::StreamExt as _;
use futures::channel::mpsc;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use server_fn::ServerFnError;
use server_fn::codec::TextStream;
use tracing::warn;

use crate::api::shared::terminal_schema::RegisterTerminalMode;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::terminal::api::PipeMessage;
use crate::terminal_id::TerminalId;
use crate::utils::ndjson_utils::serialize_line;

static NEXT_PIPE_GENERATION: AtomicUsize = AtomicUsize::new(1);
static PIPE: Mutex<Option<Pipe>> = Mutex::new(None);

#[derive(Clone)]
struct Pipe {
    generation: usize,
    sender: mpsc::Sender<PipedStream>,
}

pub struct PipedStream {
    terminal_id: TerminalId,
    stream: TextStream,
}

pub async fn pipe() -> TextStream {
    let (pipe_tx, pipe_rx) = mpsc::channel(1);
    let generation = NEXT_PIPE_GENERATION.fetch_add(1, Relaxed);
    *PIPE.lock().expect("pipe") = Some(Pipe {
        generation,
        sender: pipe_tx,
    });
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

    let end_of_pipe = futures::stream::once(async move {
        warn!("Reached end of pipe");
        let mut pipe = PIPE.lock().expect("pipe");
        if pipe
            .as_ref()
            .is_some_and(|pipe| pipe.generation == generation)
        {
            *pipe = None;
        }
        Err(ServerFnError::ServerError(String::default()))
    })
    .filter(|_| std::future::ready(false));
    let pipe = pipe.chain(end_of_pipe);

    return TextStream::new(pipe);
}

pub async fn add_stream(
    mode: RegisterTerminalMode,
    terminal_def: TerminalDef,
) -> Result<(), AddStreamError> {
    let terminal_id = terminal_def.address.id.clone();
    let stream = crate::terminal::service::stream::stream(mode, terminal_def)
        .await
        .map_err(AddStreamError::OpenStreamError)?;
    let mut pipe = PIPE
        .lock()
        .expect("PIPE")
        .as_ref()
        .map(|pipe| pipe.sender.clone())
        .ok_or(AddStreamError::PipeClosed)?;
    pipe.send(PipedStream {
        terminal_id,
        stream,
    })
    .await
    .map_err(AddStreamError::SendError)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum AddStreamError {
    #[error("[{n}] Failed to open terminal stream: {0}", n = self.name())]
    OpenStreamError(ServerFnError),

    #[error("[{n}] Failed to enqueue terminal stream: {0}", n = self.name())]
    SendError(mpsc::SendError),

    #[error("[{n}] Pipe was closed", n = self.name())]
    PipeClosed,
}
