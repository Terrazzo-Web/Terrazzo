#![doc = include_str!("../README.md")]

use std::sync::Arc;
use std::task::Poll;

use futures::Stream;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use pin_project::pin_project;
use tokio_util::io::ReaderStream;

use self::command::Command;
use self::command::SpawnError;
use self::pty::OwnedWritePty;
use self::pty::Pty;
use self::pty::PtyError;
use self::replay::Journal;
use self::replay::Reader;
use self::replay::Worker;

mod command;
pub mod lease;
pub mod pty;
mod raw_pts;
mod raw_pty;
mod release_on_drop;
pub mod replay;
pub mod size;

pub const TERRAZZO_CLIENT_NAME: &str = "TERRAZZO_CLIENT_NAME";

pub struct ProcessIO {
    input: OwnedWritePty,
    output: ProcessOutput,
    #[cfg_attr(not(test), expect(unused))]
    child_process: tokio::process::Child,
}

#[pin_project]
pub struct ProcessInput(#[pin] pub OwnedWritePty);

#[pin_project]
pub struct ProcessOutput {
    #[pin]
    pub reader: Reader,
    pub journal: Journal,
    pub worker: Arc<Worker>,
    pub stopped: Arc<std::sync::atomic::AtomicBool>,
    pub stop_waker: Arc<futures::task::AtomicWaker>,
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum OpenProcessError {
    #[error("[{n}] {0}", n = self.name())]
    PtyProcessError(#[from] PtyError),

    #[error("[{n}] {0}", n = self.name())]
    SpawnError(#[from] SpawnError),

    #[error("[{n}] Not found", n = self.name())]
    NotFound,
}

impl ProcessIO {
    pub async fn open(
        client_name: Option<impl AsRef<str>>,
        scrollback: usize,
        terminal_shell: Option<String>,
    ) -> Result<Self, OpenProcessError> {
        let pty = Pty::new()?;
        let mut command = if let Some(terminal_shell) = terminal_shell {
            let mut command = Command::new("/bin/bash");
            command.arg("-lc").arg(terminal_shell);
            command
        } else {
            let mut command =
                std::env::var("SHELL").map_or_else(|_| Command::new("/bin/bash"), Command::new);
            command.arg("-i");
            command
        };
        if let Some(client_name) = client_name {
            command.env(TERRAZZO_CLIENT_NAME, client_name.as_ref());
        }
        let pts = pty.pts()?;
        pty.resize(size::Size::new(24, 80))?;
        let child = command.spawn(&pts)?;

        // https://forums.developer.apple.com/forums/thread/734230
        pty.set_nonblocking()?;

        return Ok(Self::new(pty, child, scrollback));
    }

    fn new(pty: Pty, child_process: tokio::process::Child, scrollback: usize) -> Self {
        let (output, input) = pty.into_split();
        let output = ReaderStream::with_capacity(output, scrollback);
        let (journal, worker) = replay::spawn(output, scrollback);
        let output = ProcessOutput {
            reader: journal.reader(0),
            journal,
            worker,
            stopped: Arc::default(),
            stop_waker: Arc::default(),
        };
        Self {
            input,
            output,
            child_process,
        }
    }

    pub fn split(self) -> (ProcessInput, ProcessOutput) {
        (ProcessInput(self.input), self.output)
    }
}

impl tokio::io::AsyncWrite for ProcessInput {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.project().0.poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.project().0.poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        self.project().0.poll_shutdown(cx)
    }

    fn poll_write_vectored(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        self.project().0.poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }
}

impl Stream for ProcessOutput {
    type Item = std::io::Result<replay::Record>;
    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        self.project().reader.poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn open() {
        let mut process = super::ProcessIO::open(Option::<String>::None, 1000, None)
            .await
            .unwrap();
        process.child_process.kill().await.unwrap();
    }
}
