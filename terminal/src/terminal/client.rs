use futures::StreamExt as _;
use futures::channel::oneshot;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use server_fn::ServerFnError;
use terrazzo::prelude::XSignal;
use terrazzo::prelude::XString;
use terrazzo::prelude::diagnostics;

use self::diagnostics::debug;
use self::diagnostics::info;
use self::diagnostics::warn;
use super::api::LeaseMessage;
use crate::api::client_address::ClientAddress;
use crate::api::shared::terminal_schema::*;
use crate::terminal::ui::TerminalsState;
use crate::tiles::id::TileId;
use crate::utils::ndjson::NdjsonBuffer;
use crate::utils::watch;

pub type LiveTerminalDef = TerminalDefImpl<XSignal<TabTitle<XString>>>;

pub async fn list() -> Result<Vec<TerminalDef>, ServerFnError> {
    super::api::list().await
}

pub async fn new_id(address: ClientAddress, tile: TileId) -> Result<TerminalDef, ServerFnError> {
    super::api::new_id(address, tile).await
}

pub async fn write(
    terminal: &TerminalAddress,
    attachment: Attachment,
    data: String,
) -> Result<(), ServerFnError> {
    super::api::write(
        terminal.clone(),
        attachment,
        vec![TerminalInput::Text(data)],
    )
    .await
}

pub async fn resize(
    terminal: &TerminalAddress,
    size: Size,
    attachment: Attachment,
) -> Result<(), ServerFnError> {
    super::api::resize(ResizeRequest {
        terminal: terminal.clone(),
        size,
        attachment,
    })
    .await
}

pub async fn set_title(
    terminal: &TerminalAddress,
    title: TabTitle<String>,
) -> Result<(), ServerFnError> {
    super::api::set_title(SetTitleRequest {
        terminal: terminal.clone(),
        title,
    })
    .await
}

pub async fn set_order(tabs: Vec<TerminalAddress>) -> Result<(), ServerFnError> {
    super::api::set_order(tabs).await
}

pub async fn stream<F, F0>(
    state: TerminalsState,
    terminal_def: TerminalDef,
    notify_mouse: watch::WatchRx,
    on_init: impl FnOnce() -> F0,
    on_data: impl Fn(String) -> F,
) -> Result<(), StreamError>
where
    F: Future<Output = ()>,
    F0: Future<Output = ()>,
{
    let terminal_id = terminal_def.address.id.clone();
    let mut resume = None;
    let mut on_init = Some(on_init);
    let mut attachment = None;
    let mut cursor = AppliedCursor {
        event: "0".into(),
        end: "0".into(),
    };
    let mut allow_create = true;
    let mut restoring = false;
    let mut request_id = new_request_id();
    'connect: loop {
        let request = AttachRequest {
            request_id: request_id.clone(),
            version: 1,
            create_if_missing: allow_create,
            resume: resume.clone(),
        };
        let stream = match super::api::stream(request, terminal_def.clone()).await {
            Ok(stream) => stream,
            Err(error) => {
                let text = error.to_string();
                if text.contains("FreshRestoreRequired") {
                    request_id = new_request_id();
                    resume = None;
                    continue;
                }
                if text.contains("ProcessGone")
                    || text.contains("UnsupportedProtocol")
                    || text.contains("InvalidCursor")
                {
                    return Err(error.into());
                }
                if text.contains("StaleAttachment") {
                    on_data("\"Revoked\"".into()).await;
                    return Ok(());
                }
                warn!("Terminal reconnect failed: {error}");
                on_data("\"Disconnected\"".into()).await;
                if notify_mouse.notified().await.is_err() {
                    return Err(StreamError::UxClosed);
                }
                continue;
            }
        };
        let mut stream = stream.ready_chunks(64);
        let mut parser = NdjsonBuffer::<LeaseMessage>::default();
        'read: while let Some(chunks) = stream.next().await {
            let mut messages = vec![];
            for chunk in chunks {
                let chunk = match chunk {
                    Ok(chunk) => chunk,
                    Err(error) if error.to_string().contains("FreshRestoreRequired") => {
                        request_id = new_request_id();
                        resume = None;
                        continue 'connect;
                    }
                    Err(error) => {
                        let text = error.to_string();
                        if text.contains("ProcessGone")
                            || text.contains("UnsupportedProtocol")
                            || text.contains("InvalidCursor")
                        {
                            return Err(error.into());
                        }
                        if text.contains("StaleAttachment") {
                            on_data("\"Revoked\"".into()).await;
                            return Ok(());
                        }
                        warn!("Terminal transport interrupted: {error}");
                        break 'read;
                    }
                };
                for message in parser.push_chunk(&chunk) {
                    messages.push(message?);
                }
            }
            on_data(serde_json::to_string(&messages)?).await;
            let mut acknowledge = false;
            for message in messages {
                match message {
                    LeaseMessage::Init {
                        attachment: next,
                        checkpoint,
                        fresh,
                        ..
                    } => {
                        if fresh {
                            cursor = AppliedCursor {
                                event: "0".into(),
                                end: "0".into(),
                            };
                        }
                        request_id = new_request_id();
                        allow_create = false;
                        restoring = checkpoint.is_some();
                        attachment = Some(next);
                        if let Some(checkpoint) = checkpoint {
                            cursor = checkpoint.cursor;
                        }
                        continue;
                    }
                    LeaseMessage::SnapshotChunk(_) => continue,
                    LeaseMessage::SnapshotEnd => {
                        restoring = false;
                        continue;
                    }
                    LeaseMessage::Output { cursor: next, .. }
                    | LeaseMessage::Gap { cursor: next, .. } => cursor = next,
                    LeaseMessage::Resize { cursor: next, .. } => cursor.event = next.event,
                    LeaseMessage::Ready => {
                        if let Some(on_init) = on_init.take() {
                            on_init().await;
                        }
                    }
                    LeaseMessage::Revoked => return Ok(()),
                    LeaseMessage::Eos => {
                        state.on_eos(&terminal_id);
                        return Ok(());
                    }
                    LeaseMessage::Error(error) => return Err(StreamError::ServerFn(error)),
                }
                acknowledge = true;
            }
            if acknowledge && let Some(attachment) = &attachment {
                if let Err(error) = super::api::ack(
                    terminal_def.address.clone(),
                    attachment.clone(),
                    cursor.clone(),
                )
                .await
                {
                    let text = error.to_string();
                    if text.contains("StaleAttachment") {
                        on_data("\"Revoked\"".into()).await;
                        return Ok(());
                    }
                    if text.contains("ProcessGone") || text.contains("InvalidCursor") {
                        return Err(error.into());
                    }
                    warn!("Terminal acknowledgement interrupted: {error}");
                    break 'read;
                }
            }
        }
        on_data("\"Disconnected\"".into()).await;
        debug!("Terminal stream disconnected");
        match notify_mouse.notified().await {
            Ok(()) => info!("Terminal stream reopening"),
            Err(oneshot::Canceled) => return Err(StreamError::UxClosed),
        }
        if restoring {
            resume = None;
            continue;
        }
        resume = attachment
            .clone()
            .map(|attachment| (attachment, cursor.clone()));
    }
}

fn new_request_id() -> String {
    format!(
        "{}-{}-{}",
        web_sys::js_sys::Date::now(),
        web_sys::js_sys::Math::random(),
        web_sys::js_sys::Math::random()
    )
}

pub async fn close(terminal: &TerminalAddress, _correlation_id: Option<String>) {
    super::api::close(terminal.clone())
        .await
        .unwrap_or_else(|error| warn!("Failed to close terminal: {error}"));
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum StreamError {
    #[error("[{n}] {0}", n = self.name())]
    ServerFn(String),

    #[error("[{n}] {0}", n = self.name())]
    Json(#[from] serde_json::Error),

    #[error("[{n}] UX closed", n = self.name())]
    UxClosed,
}

impl From<ServerFnError> for StreamError {
    fn from(error: ServerFnError) -> Self {
        Self::ServerFn(error.to_string())
    }
}
