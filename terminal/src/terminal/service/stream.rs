use base64::Engine as _;
use futures::Stream;
use futures::StreamExt as _;
use futures::TryStreamExt as _;
use server_fn::ServerFnError;
use server_fn::codec::TextStream;
use terrazzo_pty::OpenProcessError;
use terrazzo_pty::ProcessIO;
use terrazzo_pty::lease::LeaseItem;
use tonic::Status;
use tracing::debug;

use crate::api::shared::terminal_schema::RegisterTerminalMode;
use crate::api::shared::terminal_schema::STREAMING_WINDOW_SIZE;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::api::shared::terminal_schema::TerminalSnapshot;
use crate::backend::client_service::remote_fn_service;
use crate::backend::throttling_stream::ThrottleProcessOutput;
use crate::processes;
use crate::processes::get_processes;
use crate::terminal::api::LeaseMessage;
use crate::utils::ndjson_utils::serialize_line;

pub async fn stream(
    mode: RegisterTerminalMode,
    terminal_def: TerminalDef,
) -> Result<TextStream, ServerFnError> {
    let remote = terminal_def.address.via.clone();
    debug!(%remote, "Calling stream()");
    let stream = STREAM_FN.call(remote, (mode, terminal_def)).await?;
    let stream = stream.map_ok(|item| {
        serialize_line(&item).unwrap_or_else(|error| {
            serialize_line(&LeaseMessage::Error(error.to_string()))
                .expect("serializing a string cannot fail")
        })
    });
    Ok(TextStream::new(stream.map_err(Into::into)))
}

remote_fn_service::streaming::declare_remote_fn!(
    STREAM_FN,
    "terminal.stream",
    (RegisterTerminalMode, TerminalDef),
    LeaseMessage,
    |server, (mode, terminal_def)| {
        let server = server.to_owned();
        let terminal_id = terminal_def.address.id.clone();
        let create = mode == RegisterTerminalMode::Create;
        let stream = processes::stream::open_stream(terminal_def, create, move |_| async move {
            if !create {
                return Err(OpenProcessError::NotFound);
            }
            let server_config = &server.config().server;
            let shell = server_config.with(|config| config.terminal_shell.clone());
            ProcessIO::open(None::<String>, STREAMING_WINDOW_SIZE, shell).await
        });
        let stream = {
            let terminal_id = terminal_id.clone();
            async move {
                match stream.await {
                    Ok(mut stream) => {
                        let process = get_processes()
                            .get(&terminal_id)
                            .ok_or_else(|| Status::not_found("ProcessGone"))?
                            .1
                            .process_id;
                        let snapshot = if create {
                            stream.restore_snapshot()
                        } else {
                            None
                        };
                        let snapshot = snapshot.map(|snapshot| TerminalSnapshot {
                            process,
                            position: snapshot.position,
                            data: String::from_utf8(snapshot.data.to_vec())
                                .expect("snapshot is UTF-8"),
                        });
                        Ok((
                            ThrottleProcessOutput::new(terminal_id, stream),
                            LeaseMessage::Init {
                                process,
                                capacity: STREAMING_WINDOW_SIZE,
                                snapshot,
                            },
                        ))
                    }
                    Err(error) => Err(Status::internal(error.to_string())),
                }
            }
        };

        use futures::future::ready;
        use futures::stream::once;
        let stream = async move {
            let (stream, init) = stream.await?;
            let stream = stream.inspect(move |next| {
                if let LeaseItem::EOS = next {
                    let _removed = get_processes().remove(&terminal_id);
                }
            });
            let stream = stream.map(LeaseMessage::from);
            Ok(once(ready(init)).chain(stream).map(Ok))
        };

        let stream = helpers::is_future_stream(stream);
        let stream = once(stream).try_flatten();
        let stream = helpers::is_message_stream(stream);

        return stream;
    }
);

/// Helpers to make types more obvious
mod helpers {
    use super::*;

    pub fn is_future_stream<F, S>(future_stream: F) -> F
    where
        F: Future<Output = Result<S, Status>>,
        S: Stream<Item = Result<LeaseMessage, Status>>,
    {
        future_stream
    }

    pub fn is_message_stream<S>(message_stream: S) -> S
    where
        S: Stream<Item = Result<LeaseMessage, Status>>,
    {
        message_stream
    }
}

impl From<LeaseItem> for LeaseMessage {
    fn from(item: LeaseItem) -> Self {
        match item {
            LeaseItem::Data(data) => {
                let position = data.position;
                match String::from_utf8(Vec::from(data.bytes)) {
                    Ok(data) => Self::Utf8 { position, data },
                    Err(error) => Self::Base64 {
                        position,
                        data: base64::engine::general_purpose::STANDARD.encode(error.into_bytes()),
                    },
                }
            }
            LeaseItem::EOS => Self::Eos,
            LeaseItem::Error(error) => Self::Error(error.to_string()),
        }
    }
}
