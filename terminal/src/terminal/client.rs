use std::sync::Mutex;

use futures::FutureExt as _;
use futures::StreamExt as _;
use futures::channel::oneshot;
use futures::future::Shared;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use server_fn::ServerFnError;
use terrazzo::prelude::XSignal;
use terrazzo::prelude::XString;
use terrazzo::prelude::diagnostics;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::Closure;
use web_sys::Element;
use web_sys::MouseEvent;
use web_sys::js_sys::Uint8Array;

use self::diagnostics::warn;
use super::api::LeaseMessage;
use crate::api::client_address::ClientAddress;
use crate::api::shared::terminal_schema::*;
use crate::terminal::ui::TerminalsState;
use crate::tiles::id::TileId;
use crate::utils::ndjson::NdjsonBuffer;

pub type LiveTerminalDef = TerminalDefImpl<XSignal<TabTitle<XString>>>;

static STREAM_WAKE: Mutex<StreamWake> = Mutex::new(StreamWake {
    generation: 0,
    signal: None,
});

struct StreamWake {
    generation: usize,
    signal: Option<(oneshot::Sender<()>, Shared<oneshot::Receiver<()>>)>,
}

const WAKE_EVENT_TYPE: &str = "mousemove";

struct WakeListener {
    element: Element,
    closure: Closure<dyn Fn(MouseEvent)>,
    attached: bool,
}

impl WakeListener {
    fn new(element: Element) -> Self {
        let closure = Closure::new(move |_| wake_streams());
        let attached = element
            .add_event_listener_with_callback(WAKE_EVENT_TYPE, closure.as_ref().unchecked_ref())
            .is_ok();
        Self {
            element,
            closure,
            attached,
        }
    }
}

impl Drop for WakeListener {
    fn drop(&mut self) {
        if self.attached {
            let _ = self.element.remove_event_listener_with_callback(
                WAKE_EVENT_TYPE,
                self.closure.as_ref().unchecked_ref(),
            );
        }
    }
}

pub fn wake_streams() {
    let signal = {
        let mut wake = STREAM_WAKE.lock().expect("stream wake");
        wake.generation = wake.generation.wrapping_add(1);
        wake.signal.take()
    };
    if let Some((sender, _receiver)) = signal {
        let _ = sender.send(());
    }
}

fn current_wake_generation() -> usize {
    STREAM_WAKE.lock().expect("stream wake").generation
}

async fn wait_until_stream_is_needed(generation: usize) {
    let receiver = {
        let mut wake = STREAM_WAKE.lock().expect("stream wake");
        let receiver = match &wake.signal {
            Some((_sender, receiver)) => receiver.clone(),
            None => {
                let (sender, receiver) = oneshot::channel();
                let receiver = receiver.shared();
                wake.signal = Some((sender, receiver.clone()));
                receiver
            }
        };
        if wake.generation != generation
            && let Some((sender, _receiver)) = wake.signal.take()
        {
            let _ = sender.send(());
        }
        receiver
    };
    let _ = receiver.await;
}

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
    element: Element,
    on_init: impl FnOnce() -> F0,
    on_data: impl Fn(JsValue) -> F,
) -> Result<(), StreamError>
where
    F: Future<Output = ()>,
    F0: Future<Output = ()>,
{
    let terminal_id = terminal_def.address.id.clone();
    let _wake_listener = WakeListener::new(element);
    let mut mode = RegisterTerminalMode::Create;
    let mut on_init = Some(on_init);
    loop {
        let wake_generation = current_wake_generation();
        let mut stream = super::api::stream(mode, terminal_def.clone())
            .await
            .map_err(StreamError::from)?
            .into_inner();
        let mut parser = NdjsonBuffer::<LeaseMessage>::default();
        let mut unacked = 0;
        while let Some(chunk) = stream.next().await {
            for message in parser.push_chunk(&chunk.map_err(StreamError::from)?) {
                match message.map_err(|error| StreamError::ServerFn(error.to_string()))? {
                    LeaseMessage::Init => {
                        if let Some(on_init) = on_init.take() {
                            on_init().await;
                        }
                    }
                    LeaseMessage::Data(data) => {
                        unacked += data.len();
                        let value = Uint8Array::new_with_length(data.len() as u32);
                        value.copy_from(&data);
                        on_data(value.into()).await;
                        if unacked >= STREAMING_WINDOW_SIZE / 2 {
                            super::api::ack(
                                terminal_def.address.clone(),
                                std::mem::take(&mut unacked),
                            )
                            .await
                            .map_err(StreamError::from)?;
                        }
                    }
                    LeaseMessage::Eos => {
                        state.on_eos(&terminal_id);
                        return Ok(());
                    }
                    LeaseMessage::Error(error) => {
                        state.on_eos(&terminal_id);
                        return Err(StreamError::ServerFn(error));
                    }
                }
            }
        }
        warn!("Terminal stream disconnected; reopening");
        mode = RegisterTerminalMode::Reopen;
        wait_until_stream_is_needed(wake_generation).await;
    }
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
}

impl From<ServerFnError> for StreamError {
    fn from(error: ServerFnError) -> Self {
        Self::ServerFn(error.to_string())
    }
}
