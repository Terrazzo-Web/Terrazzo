use base64::Engine as _;
use futures::StreamExt as _;
use futures::TryStreamExt as _;
use server_fn::ServerFnError;
use server_fn::codec::TextStream;
use terrazzo_pty::OpenProcessError;
use terrazzo_pty::ProcessIO;
use terrazzo_pty::lease::LeaseItem;
use terrazzo_pty::replay::Event;
use tonic::Status;

use crate::api::shared::terminal_schema::AppliedCursor;
use crate::api::shared::terminal_schema::AttachRequest;
use crate::api::shared::terminal_schema::Attachment;
use crate::api::shared::terminal_schema::STREAMING_WINDOW_SIZE;
use crate::api::shared::terminal_schema::Size;
use crate::api::shared::terminal_schema::TerminalCheckpoint;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::backend::client_service::remote_fn_service;
use crate::backend::throttling_stream::ThrottleProcessOutput;
use crate::terminal::api::LeaseMessage;
use crate::utils::ndjson_utils::serialize_line;

pub async fn stream(
    request: AttachRequest,
    terminal_def: TerminalDef,
) -> Result<TextStream, ServerFnError> {
    let stream = STREAM_FN
        .call(terminal_def.address.via.clone(), (request, terminal_def))
        .await?;
    Ok(TextStream::new(
        stream
            .map_ok(|item| serialize_line(&item).expect("serializable terminal event"))
            .map_err(Into::into),
    ))
}

remote_fn_service::streaming::declare_remote_fn!(
    STREAM_FN,
    "terminal.stream",
    (AttachRequest, TerminalDef),
    LeaseMessage,
    |server, (request, terminal_def): (AttachRequest, TerminalDef)| {
        let server = server.clone();
        let stream = async move {
            if request.version != 1 {
                return Err(Status::failed_precondition("UnsupportedProtocol"));
            }
            let id = terminal_def.address.id.clone();
            let create = request.create_if_missing;
            if !create && !crate::processes::get_processes().contains_key(&id) {
                return Err(Status::not_found("ProcessGone"));
            }
            if let Some((attachment, cursor)) = &request.resume {
                let entry = super::checkpoint::entry(&terminal_def.address, attachment)?;
                entry
                    .journal
                    .validate_end(
                        attachment
                            .token
                            .parse()
                            .map_err(|_| Status::invalid_argument("InvalidToken"))?,
                        cursor
                            .event
                            .parse()
                            .map_err(|_| Status::invalid_argument("InvalidCursor"))?,
                        cursor
                            .end
                            .parse()
                            .map_err(|_| Status::invalid_argument("InvalidCursor"))?,
                    )
                    .map_err(|error| {
                        if error == "InvalidCursor" {
                            Status::failed_precondition("FreshRestoreRequired")
                        } else {
                            Status::failed_precondition(error)
                        }
                    })?;
            }
            let mut lease =
                crate::processes::stream::open_stream(terminal_def, false, move |_| async move {
                    if !create {
                        return Err(OpenProcessError::NotFound);
                    }
                    let shell = server
                        .config()
                        .server
                        .with(|config| config.terminal_shell.clone());
                    ProcessIO::open(None::<String>, STREAMING_WINDOW_SIZE, shell).await
                })
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            let entry = crate::processes::get_processes()
                .get(&id)
                .ok_or_else(|| Status::not_found("ProcessGone"))?
                .1
                .clone();
            let resume = match request.resume {
                Some((attachment, cursor)) => {
                    if attachment.incarnation != entry.incarnation {
                        return Err(Status::failed_precondition("FreshRestoreRequired"));
                    }
                    let token = attachment
                        .token
                        .parse()
                        .map_err(|_| Status::invalid_argument("InvalidToken"))?;
                    entry
                        .journal
                        .validate_end(
                            token,
                            cursor
                                .event
                                .parse()
                                .map_err(|_| Status::invalid_argument("InvalidCursor"))?,
                            cursor
                                .end
                                .parse()
                                .map_err(|_| Status::invalid_argument("InvalidCursor"))?,
                        )
                        .map_err(Status::invalid_argument)?;
                    Some(
                        cursor
                            .event
                            .parse()
                            .map_err(|_| Status::invalid_argument("InvalidCursor"))?,
                    )
                }
                None => None,
            };
            let input_guard = entry.input().await;
            let (token, checkpoint, cursor) = entry
                .journal
                .attach(resume)
                .map_err(Status::invalid_argument)?;
            drop(input_guard);
            lease.seek(cursor);
            let high_water = entry.journal.high_water();
            let attachment = Attachment {
                incarnation: entry.incarnation.clone(),
                token: token.to_string(),
            };
            let checkpoint = checkpoint.map(|c| TerminalCheckpoint {
                version: 1,
                attachment: attachment.clone(),
                cursor: AppliedCursor {
                    event: c.event.to_string(),
                    end: c.end.to_string(),
                },
                size: Size {
                    rows: c.rows.into(),
                    cols: c.cols.into(),
                },
                payload: String::from_utf8(c.payload.to_vec()).expect("UTF-8 checkpoint"),
            });
            let initial_end = checkpoint
                .as_ref()
                .map_or(0, |c| c.cursor.end.parse().unwrap_or(0));
            let mut chunks = vec![];
            let checkpoint = checkpoint.map(|mut checkpoint| {
                for chunk in checkpoint.payload.as_bytes().chunks(32 * 1024) {
                    chunks.push(LeaseMessage::SnapshotChunk(
                        base64::engine::general_purpose::STANDARD.encode(chunk),
                    ));
                }
                checkpoint.payload.clear();
                checkpoint
            });
            let has_checkpoint = checkpoint.is_some();
            let init = LeaseMessage::Init {
                version: 1,
                fresh: resume.is_none(),
                attachment,
                checkpoint,
                capacity: STREAMING_WINDOW_SIZE,
            };
            let mut ready = cursor >= high_water;
            let mut initial = vec![init];
            initial.extend(chunks);
            if has_checkpoint {
                initial.push(LeaseMessage::SnapshotEnd);
            }
            if ready {
                initial.push(LeaseMessage::Ready);
            }
            let journal = entry.journal.clone();
            let stream = ThrottleProcessOutput::new(id.clone(), token, initial_end, cursor, lease)
                .flat_map(move |item| {
                    let mut messages = vec![];
                    match item {
                        LeaseItem::Data(record) => {
                            if !journal.mark_sent(token, &record) {
                                return futures::stream::iter(vec![LeaseMessage::Revoked]);
                            }
                            let event_id = record.id;
                            messages.push(match record.event {
                                Event::Output { end, bytes } => LeaseMessage::Output {
                                    cursor: AppliedCursor {
                                        event: event_id.to_string(),
                                        end: end.to_string(),
                                    },
                                    base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                                },
                                Event::Resize { rows, cols } => LeaseMessage::Resize {
                                    cursor: AppliedCursor {
                                        event: event_id.to_string(),
                                        end: String::new(),
                                    },
                                    size: Size {
                                        rows: rows.into(),
                                        cols: cols.into(),
                                    },
                                },
                                Event::Gap { end, rows, cols } => LeaseMessage::Gap {
                                    cursor: AppliedCursor {
                                        event: event_id.to_string(),
                                        end: end.to_string(),
                                    },
                                    size: Size {
                                        rows: rows.into(),
                                        cols: cols.into(),
                                    },
                                },
                            });
                            if !ready && event_id >= high_water {
                                ready = true;
                                messages.push(LeaseMessage::Ready);
                            }
                        }
                        LeaseItem::EOS => {
                            crate::processes::get_processes().remove(&id);
                            messages.push(LeaseMessage::Eos);
                        }
                        LeaseItem::Error(error) => {
                            messages.push(LeaseMessage::Error(error.to_string()))
                        }
                    }
                    futures::stream::iter(messages)
                });
            Ok(futures::stream::iter(initial)
                .chain(stream)
                .chain(futures::stream::once(async { LeaseMessage::Revoked }))
                .map(Ok::<_, Status>))
        };
        futures::stream::once(stream).try_flatten()
    }
);
