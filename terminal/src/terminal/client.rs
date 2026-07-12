use self::diagnostics::{debug, warn};
use super::api::LeaseMessage;
use crate::api::shared::terminal_schema::*;
use crate::terminal::ui::TerminalsState;
use crate::terminal_id::TerminalId;
use crate::tiles::id::TileId;
use crate::utils::ndjson::NdjsonBuffer;
use futures::StreamExt as _;
use nameth::{NamedEnumValues as _, nameth};
use scopeguard::defer;
use server_fn::ServerFnError;
use std::sync::Arc;
use terrazzo::prelude::{XSignal, XString, diagnostics};
use wasm_bindgen::JsValue;
use web_sys::{Element, js_sys::Uint8Array};

pub type LiveTerminalDef = TerminalDefImpl<XSignal<TabTitle<XString>>>;
pub mod list {
    use super::*;
    pub async fn list() -> Result<Vec<TerminalDef>, ServerFnError> {
        super::super::api::list().await
    }
}
pub mod new_id {
    use super::*;
    pub async fn new_id(
        address: crate::api::client_address::ClientAddress,
        tile: TileId,
    ) -> Result<TerminalDef, ServerFnError> {
        super::super::api::new_id(address, tile).await
    }
}
pub mod write {
    use super::*;
    pub async fn write(terminal: &TerminalAddress, data: String) -> Result<(), ServerFnError> {
        super::super::api::write(WriteRequest {
            terminal: terminal.clone(),
            data,
        })
        .await
    }
}
pub mod resize {
    use super::*;
    pub async fn resize(
        terminal: &TerminalAddress,
        size: Size,
        force: bool,
    ) -> Result<(), ServerFnError> {
        super::super::api::resize(ResizeRequest {
            terminal: terminal.clone(),
            size,
            force,
        })
        .await
    }
}
pub mod set_title {
    use super::*;
    pub async fn set_title(
        terminal: &TerminalAddress,
        title: TabTitle<String>,
    ) -> Result<(), ServerFnError> {
        super::super::api::set_title(SetTitleRequest {
            terminal: terminal.clone(),
            title,
        })
        .await
    }
}
pub mod set_order {
    use super::*;
    pub async fn set_order(tabs: Vec<TerminalAddress>) -> Result<(), ServerFnError> {
        super::super::api::set_order(tabs).await
    }
}

pub mod stream {
    use super::*;
    pub async fn stream<F, F0>(
        state: TerminalsState,
        terminal_def: TerminalDef,
        _element: Element,
        on_init: impl FnOnce() -> F0,
        on_data: impl Fn(JsValue) -> F,
    ) -> Result<(), StreamError>
    where
        F: Future<Output = ()>,
        F0: Future<Output = ()>,
    {
        let terminal_id = terminal_def.address.id.clone();
        defer! { state.on_eos(&terminal_id); }
        let mut mode = RegisterTerminalMode::Create;
        on_init().await;
        loop {
            let request = RegisterTerminalRequest {
                mode,
                def: terminal_def.clone(),
            };
            let mut stream = super::super::api::stream(request)
                .await
                .map_err(StreamError::from)?
                .into_inner();
            let mut parser = NdjsonBuffer::<LeaseMessage>::default();
            let mut unacked = 0;
            while let Some(chunk) = stream.next().await {
                for message in parser.push_chunk(&chunk.map_err(StreamError::from)?) {
                    match message.map_err(|error| StreamError::ServerFn(error.to_string()))? {
                        LeaseMessage::Data(data) => {
                            unacked += data.len();
                            let value = Uint8Array::new_with_length(data.len() as u32);
                            value.copy_from(&data);
                            on_data(value.into()).await;
                            if unacked >= STREAMING_WINDOW_SIZE / 2 {
                                super::super::api::ack(AckRequest {
                                    terminal: terminal_def.address.clone(),
                                    ack: std::mem::take(&mut unacked),
                                })
                                .await
                                .map_err(StreamError::from)?;
                            }
                        }
                        LeaseMessage::Eos => return Ok(()),
                        LeaseMessage::Error(error) => return Err(StreamError::ServerFn(error)),
                    }
                }
            }
            warn!("Terminal stream disconnected; reopening");
            mode = RegisterTerminalMode::Reopen;
        }
    }
    pub async fn close(terminal: &TerminalAddress, _correlation_id: Option<String>) {
        super::super::api::close(terminal.clone())
            .await
            .unwrap_or_else(|error| warn!("Failed to close terminal: {error}"));
    }
    pub fn drop_dispatcher(_terminal_id: &TerminalId) -> Option<Arc<str>> {
        None
    }
    pub async fn close_pipe(_correlation_id: Arc<str>) {}
    pub fn try_restart_pipe() {
        debug!("Terminal streams reconnect automatically")
    }
    #[nameth]
    #[derive(thiserror::Error, Debug)]
    pub enum StreamError {
        #[error("[{n}] {0}", n = self.name())]
        ServerFn(String),
    }
    impl From<ServerFnError> for StreamError {
        fn from(error: ServerFnError) -> Self {
            Self::ServerFn(error.to_string())
        }
    }
}
