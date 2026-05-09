#![doc = include_str!("../README.md")]

use std::task::Poll;
use std::task::ready;

use bytes::Bytes;
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
use self::tail::TailStream;

mod command;
pub mod lease;
pub mod pty;
mod raw_pts;
mod raw_pty;
mod release_on_drop;
pub mod size;
mod tail;

pub const TERRAZZO_CLIENT_NAME: &str = "TERRAZZO_CLIENT_NAME";

pub struct ProcessIO {
    input: OwnedWritePty,
    output: TailStream,
    #[expect(unused)]
    child_process: tokio::process::Child,
}

#[pin_project]
pub struct ProcessInput(#[pin] pub OwnedWritePty);

#[pin_project]
pub struct ProcessOutput(#[pin] pub TailStream);

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
        run_as: Option<impl AsRef<str>>,
        scrollback: usize,
    ) -> Result<Self, OpenProcessError> {
        let pty = Pty::new()?;
        let mut command = if let Some(run_as) = &run_as {
            let mut command = Command::new("sudo");
            command.arg("-Hiu").arg(run_as.as_ref()).arg("env");
            command
        } else {
            let mut command =
                std::env::var("SHELL").map_or_else(|_| Command::new("/bin/bash"), Command::new);
            command.arg("-i");
            command
        };
        if let Some(client_name) = client_name {
            if run_as.is_some() {
                let client_name = client_name.as_ref().replace(|c: char| !c.is_ascii(), "");
                command.arg(format!("{TERRAZZO_CLIENT_NAME}={client_name}"));
            } else {
                command.env(TERRAZZO_CLIENT_NAME, client_name.as_ref());
            }
        }
        if run_as.is_some() {
            command
                .arg("/bin/sh")
                .arg("-lc")
                .arg("exec \"${SHELL:-/bin/bash}\" -i");
        }
        let child = command.spawn(&pty.pts()?)?;

        // https://forums.developer.apple.com/forums/thread/734230
        pty.set_nonblocking()?;

        return Ok(Self::new(pty, child, scrollback));
    }

    fn new(pty: Pty, child_process: tokio::process::Child, scrollback: usize) -> Self {
        let (output, input) = pty.into_split();
        let output = ReaderStream::with_capacity(output, scrollback);
        let output = TailStream::new(output, scrollback);
        Self {
            input,
            output,
            child_process,
        }
    }

    pub fn split(self) -> (ProcessInput, ProcessOutput) {
        (ProcessInput(self.input), ProcessOutput(self.output))
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
    type Item = std::io::Result<Bytes>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match ready!(self.project().0.poll_next(cx)) {
            Some(Ok(bytes)) if !bytes.is_empty() => Some(Ok(bytes)),
            Some(Err(error)) => Some(Err(error)),
            _ => None,
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn open() {
        super::ProcessIO::open(Option::<String>::None, Option::<String>::None, 1000)
            .await
            .unwrap();
    }
}
