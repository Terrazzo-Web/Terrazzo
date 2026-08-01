use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::Mutex;

use futures::FutureExt as _;
use futures::StreamExt as _;
use futures::channel::mpsc;
use futures::channel::oneshot;
use futures::future::Shared;
use get::TerminalStream;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use scopeguard::defer;
use terrazzo::prelude::OrElseLog as _;
use terrazzo::prelude::diagnostics;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::Closure;
use web_sys::Element;
use web_sys::MouseEvent;
use web_sys::js_sys::Uint8Array;

use self::diagnostics::debug;
use self::diagnostics::info;
use self::diagnostics::warn;
use self::get::get;
use self::register::RegisterError;
use crate::api::client::request::SendRequestError;
use crate::api::shared::terminal_schema::RegisterTerminalMode;
use crate::api::shared::terminal_schema::RegisterTerminalRequest;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::terminal::ui::TerminalsState;
use crate::terminal_id::TerminalId;

mod ack;
mod close;
mod dispatch;
mod get;
mod keepalive;
mod pipe;
mod register;

pub use self::close::close;
pub use self::close::drop_dispatcher;
pub use self::pipe::close_pipe;

static GLOBAL_AWAKE: Mutex<GlobalAwake> = Mutex::new(GlobalAwake {
    generation: 0,
    signal: None,
});

struct GlobalAwake {
    generation: usize,
    signal: Option<(oneshot::Sender<()>, Shared<oneshot::Receiver<()>>)>,
}

/// Pumps data into XTermJS.
///
/// EOS flow:
/// 1. Stream ends or fails
/// 2. [Close the terminal](TerminalsState::close_terminal)
/// 3. Tab is removed from UI
/// 4. [Close the stream](fn@crate::api::client::stream::close)
///    - Take the dispatcher out of the map
///    - If the map is empty, add the correlation id
/// 5. */Server side/* Close the process
/// 6. */Server side/* If there is a correlation id, drop the registration
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
    defer! { state.on_eos(&terminal_id); }
    let _wake_listener = WakeListener::new(element);
    let query = RegisterTerminalRequest {
        mode: RegisterTerminalMode::Create,
        def: terminal_def,
    };
    let mut reader = get(query.clone()).await?;

    debug!("On init");
    let () = on_init().await;

    debug!("Streaming");
    loop {
        let wake_generation = current_wake_generation();
        match do_stream(reader, &on_data).await {
            StreamStatus::PipeDisconnected => (),
            StreamStatus::EndOfStream => return Ok(()),
        };
        info!("Streaming stopped");

        let rx = {
            let mut global_awake = GLOBAL_AWAKE.lock().or_throw("GLOBAL_AWAKE");
            match &global_awake.signal {
                Some((_tx, rx)) => rx.clone(),
                None => {
                    let (tx, rx) = oneshot::channel();
                    let rx = rx.shared();
                    global_awake.signal = Some((tx, rx.clone()));
                    rx
                }
            }
        };
        if current_wake_generation() != wake_generation {
            debug!("Mouse moved before the pipe disconnect was observed");
            try_restart_pipe();
        }

        match rx.await {
            Ok(()) => debug!("Wake-up to continue streaming"),
            Err(oneshot::Canceled) => {
                debug!("Not restarting stream, terminal is canceled");
                return Ok(());
            }
        }
        let query = RegisterTerminalRequest {
            mode: RegisterTerminalMode::Reopen,
            def: query.def.clone(),
        };
        let Ok(new_reader) = get(query).await else {
            warn!("Can't re-open the stream");
            return Ok(());
        };
        info!("Re-opened the stream");
        reader = new_reader;
    }
}

async fn do_stream<F>(
    mut reader: impl TerminalStream,
    on_data: &impl Fn(JsValue) -> F,
) -> StreamStatus
where
    F: Future<Output = ()>,
{
    while let Some(chunks) = reader.next().await {
        let chunk = {
            let capacity = chunks
                .iter()
                .filter_map(|chunk| chunk.as_ref().map(Vec::len))
                .sum();
            let mut concat = Vec::with_capacity(capacity);
            for chunk in chunks {
                let Some(chunk) = chunk else {
                    debug!("End of stream");
                    return StreamStatus::EndOfStream;
                };
                concat.extend_from_slice(&chunk);
            }
            concat
        };
        let js_value = Uint8Array::new_with_length(chunk.len() as u32);
        js_value.copy_from(&chunk);
        let () = on_data(js_value.into()).await;
    }
    return StreamStatus::PipeDisconnected;
}

#[must_use]
enum StreamStatus {
    PipeDisconnected,
    EndOfStream,
}

const WAKE_EVENT_TYPE: &str = "mousemove";

struct WakeListener {
    element: Element,
    closure: Closure<dyn Fn(MouseEvent)>,
    attached: bool,
}

impl WakeListener {
    fn new(element: Element) -> Self {
        let closure = Closure::new(move |_| {
            let signal = {
                let mut global_awake = GLOBAL_AWAKE.lock().or_throw("GLOBAL_AWAKE");
                global_awake.generation = global_awake.generation.wrapping_add(1);
                global_awake.signal.take()
            };
            send_restart_signal(signal);
        });
        let attached = element
            .add_event_listener_with_callback(WAKE_EVENT_TYPE, closure.as_ref().unchecked_ref())
            .inspect_err(|error| warn!("Unable to attach mouse move event handler: {error:?}"))
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
        if !self.attached {
            return;
        }
        let function = self.closure.as_ref().unchecked_ref();
        let () = self
            .element
            .remove_event_listener_with_callback(WAKE_EVENT_TYPE, function)
            .unwrap_or_else(|error| warn!("Failed to remove event handler: {error:?}"));
    }
}

fn current_wake_generation() -> usize {
    GLOBAL_AWAKE.lock().or_throw("GLOBAL_AWAKE").generation
}

pub fn try_restart_pipe() {
    let signal = GLOBAL_AWAKE.lock().or_throw("GLOBAL_AWAKE").signal.take();
    send_restart_signal(signal);
}

fn send_restart_signal(signal: Option<(oneshot::Sender<()>, Shared<oneshot::Receiver<()>>)>) {
    let Some((tx, _rx)) = signal else { return };
    let _ = tx.send(());
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum StreamError {
    #[error("[{n}] {0}", n = self.name())]
    SendRequestError(#[from] SendRequestError),

    #[error("[{n}] {0}", n = self.name())]
    RegisterError(#[from] RegisterError),
}

struct StreamDispatchers {
    correlation_id: Arc<str>,
    map: HashMap<TerminalId, mpsc::Sender<Option<Vec<u8>>>>,
    shutdown_pipe: ShutdownPipe,
}

enum ShutdownPipe {
    Pending(Shared<oneshot::Receiver<()>>),
    Signal(oneshot::Sender<()>),
}

static DISPATCHERS: Dispatchers = Dispatchers::new();

struct Dispatchers(Mutex<Option<StreamDispatchers>>);

impl Dispatchers {
    pub const fn new() -> Self {
        Self(Mutex::new(None))
    }
}

impl Deref for Dispatchers {
    type Target = Mutex<Option<StreamDispatchers>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
