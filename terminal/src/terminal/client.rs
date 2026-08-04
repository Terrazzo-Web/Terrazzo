use base64::Engine as _;
use futures::StreamExt as _;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use server_fn::ServerFnError;
use terrazzo::prelude::XSignal;
use terrazzo::prelude::XString;
use terrazzo::prelude::diagnostics;
use wasm_bindgen::JsValue;
use web_sys::js_sys::Uint8Array;

use self::diagnostics::warn;
use super::api::LeaseMessage;
use crate::api::client_address::ClientAddress;
use crate::api::shared::terminal_schema::*;
use crate::terminal::ui::TerminalsState;
use crate::tiles::id::TileId;
use crate::utils::ndjson::NdjsonBuffer;

pub type LiveTerminalDef = TerminalDefImpl<XSignal<TabTitle<XString>>>;

pub async fn list() -> Result<Vec<TerminalDef>, ServerFnError> {
    super::api::list().await
}

pub async fn new_id(address: ClientAddress, tile: TileId) -> Result<TerminalDef, ServerFnError> {
    super::api::new_id(address, tile).await
}

pub async fn write(terminal: &TerminalAddress, data: String) -> Result<(), ServerFnError> {
    super::api::write(terminal.clone(), data).await
}

pub async fn resize(
    terminal: &TerminalAddress,
    size: Size,
    force: bool,
) -> Result<(), ServerFnError> {
    super::api::resize(ResizeRequest {
        terminal: terminal.clone(),
        size,
        force,
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
    on_init: impl FnOnce() -> F0,
    on_data: impl Fn(JsValue) -> F,
) -> Result<(), StreamError>
where
    F: Future<Output = ()>,
    F0: Future<Output = ()>,
{
    let terminal_id = terminal_def.address.id.clone();
    let mut mode = RegisterTerminalMode::Create;
    let mut on_init = Some(on_init);
    loop {
        let mut stream = super::api::stream(mode, terminal_def.clone())
            .await?
            .into_inner();
        let mut parser = NdjsonBuffer::<LeaseMessage>::default();
        let mut unacked = 0;
        while let Some(chunk) = stream.next().await {
            let messages: Vec<Result<LeaseMessage, serde_json::Error>> =
                parser.push_chunk(&chunk.map_err(StreamError::from)?);
            let mut buffer = vec![];
            for message in messages {
                match message.map_err(StreamError::Json)? {
                    LeaseMessage::Init => {
                        if let Some(on_init) = on_init.take() {
                            on_init().await;
                        }
                    }
                    LeaseMessage::Eos => {
                        process_data(&terminal_def, &on_data, &mut unacked, &buffer).await?;
                        state.on_eos(&terminal_id);
                        return Ok(());
                    }
                    LeaseMessage::Error(error) => {
                        process_data(&terminal_def, &on_data, &mut unacked, &buffer).await?;
                        state.on_eos(&terminal_id);
                        return Err(StreamError::ServerFn(error));
                    }
                    LeaseMessage::Base64(data) => {
                        let data = base64::engine::general_purpose::STANDARD.decode(data)?;
                        buffer.extend(data);
                    }
                    LeaseMessage::Utf8(data) => {
                        buffer.extend(data.as_bytes());
                    }
                }
            }
            process_data(&terminal_def, &on_data, &mut unacked, &buffer).await?;
        }
        warn!("Terminal stream disconnected; reopening");
        mode = RegisterTerminalMode::Reopen;
    }
}

async fn process_data<F>(
    terminal_def: &TerminalDefImpl<TabTitle<String>>,
    on_data: &impl Fn(JsValue) -> F,
    unacked: &mut usize,
    data: &[u8],
) -> Result<(), StreamError>
where
    F: Future<Output = ()>,
{
    if data.is_empty() {
        return Ok(());
    }
    *unacked += data.len();
    let value = Uint8Array::new_with_length(data.len() as u32);
    value.copy_from(data);
    on_data(value.into()).await;
    if *unacked >= STREAMING_WINDOW_SIZE / 2 {
        super::api::ack(terminal_def.address.clone(), std::mem::take(unacked))
            .await
            .map_err(StreamError::from)?;
    }
    Ok(())
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

    #[error("[{n}] {0}", n = self.name())]
    DecodeError(#[from] base64::DecodeError),
}

impl From<ServerFnError> for StreamError {
    fn from(error: ServerFnError) -> Self {
        Self::ServerFn(error.to_string())
    }
}
