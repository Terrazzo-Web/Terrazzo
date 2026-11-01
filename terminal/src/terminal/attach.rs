use std::future::ready;

use futures::FutureExt as _;
use futures::SinkExt as _;
use futures::StreamExt as _;
use futures::channel::mpsc;
use futures::channel::oneshot;
use futures::select;
use scopeguard::defer;
use terrazzo::prelude::with_generation_id::WithGenerationId;
use terrazzo::prelude::*;
use terrazzo::widgets::resize_event::ResizeEvent;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;

use self::diagnostics::Instrument as _;
use self::diagnostics::debug;
use self::diagnostics::debug_span;
use self::diagnostics::error;
use self::diagnostics::info;
use self::diagnostics::info_span;
use self::diagnostics::span::Span;
use self::diagnostics::warn;
use super::client as terminal_api;
use super::javascript::TerminalJs;
use super::javascript::TerminalJsRc;
use super::terminal_tab::TerminalTab;
use super::ui::TerminalsState;
use crate::api::shared::terminal_schema;
use crate::api::shared::terminal_schema::TabTitle;
use crate::api::shared::terminal_schema::TerminalAddress;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::utils::watch;

const XTERMJS_ATTR: &str = "data-xtermjs";
const IS_ATTACHED: &str = "Y";

pub fn attach(
    template: XTemplate,
    state: TerminalsState,
    terminal_tab: TerminalTab,
    notify_mouse: watch::WatchRx,
) -> Consumers {
    let terminal_address = terminal_tab.address.to_owned();
    let terminal_id = terminal_address.id.clone();
    let terminal_def = terminal_tab.to_terminal_def();
    let _span = info_span!("XTermJS", %terminal_id).entered();
    let element = template.element();
    if let Some(IS_ATTACHED) = element.get_attribute(XTERMJS_ATTR).as_deref() {
        if terminal_tab.selected.get_value_untracked()
            && let Some(xtermjs) = terminal_tab
                .xtermjs
                .lock()
                .or_throw("xtermjs.lock()")
                .clone()
        {
            info!("Focus and fit size");
            xtermjs.focus();
            xtermjs.fit();
        }
        return Consumers::default();
    }
    element
        .set_attribute(XTERMJS_ATTR, IS_ATTACHED)
        .or_throw(XTERMJS_ATTR);

    info!("Attaching XtermJS");
    let xtermjs = WithGenerationId::from(TerminalJsRc::new());
    *terminal_tab.xtermjs.lock().or_throw("xtermjs") = Some(xtermjs.clone());
    let attachment_cancel = make_attachment_cancel(&terminal_tab);
    xtermjs.open(&element);
    let (input_tx, input_rx) = mpsc::unbounded();
    let on_data = xtermjs.do_on_data(input_tx);
    let on_resize = xtermjs.do_on_resize(terminal_address.clone());
    let on_checkpoint = xtermjs.do_on_checkpoint(terminal_address.clone());
    let on_title_change = xtermjs.do_on_title_change(terminal_tab.title.clone());
    let selected = terminal_tab.selected.get_value_untracked();
    let io = async move {
        let (initialized_tx, initialized_rx) = oneshot::channel();
        let stream_loop = xtermjs.stream_loop(state, terminal_def, initialized_tx, notify_mouse);
        let write_loop = write_loop(&terminal_address, &xtermjs, input_rx, initialized_rx);
        let unsubscribe_resize_event = ResizeEvent::signal().add_subscriber({
            let xtermjs = xtermjs.clone();
            move |_| xtermjs.fit()
        });
        if selected {
            xtermjs.focus();
        }
        select! {
            () = stream_loop.fuse() => info!("Stream loop closed"),
            () = write_loop.fuse() => info!("Write loop closed"),
            _ = attachment_cancel.fuse() => info!("Attachment replaced"),
        };
        {
            let mut lock = terminal_tab.xtermjs.lock().or_throw("xtermjs");
            if let Some(xtermjs_old) = &*lock
                && xtermjs_old.generation_id <= xtermjs.generation_id
            {
                *lock = None;
            }
        }
        drop(unsubscribe_resize_event);
        drop(on_checkpoint);
        drop(on_title_change);
        drop(on_resize);
        drop(on_data);
        drop(xtermjs);
        info!("Detached XtermJS");
    };
    spawn_local(io.in_current_span());
    return Consumers::default();
}

fn make_attachment_cancel(terminal_tab: &TerminalTab) -> oneshot::Receiver<()> {
    let (attachment_cancel_tx, attachment_cancel_rx) = oneshot::channel();
    if let Some(previous) = terminal_tab
        .attachment_cancel
        .lock()
        .or_throw("attachment_cancel")
        .replace(attachment_cancel_tx)
    {
        let _ = previous.send(());
    }
    attachment_cancel_rx
}

impl TerminalJs {
    fn do_on_data(
        &self,
        input_tx: mpsc::UnboundedSender<terminal_schema::TerminalInput>,
    ) -> Closure<dyn FnMut(JsValue)> {
        let span = Span::current();
        let on_data: Closure<dyn FnMut(JsValue)> = Closure::new(move |data: JsValue| {
            let mut input_tx = input_tx.clone();
            let data = match data.as_string() {
                Some(text) => terminal_schema::TerminalInput::Text(text),
                None => terminal_schema::TerminalInput::Binary(
                    web_sys::js_sys::Uint8Array::new(&data).to_vec(),
                ),
            };
            let send = async move {
                let result = input_tx.send(data).await;
                // The channel is unbounded, the only possible error is the write_loop has dropped.
                return result.unwrap_or_else(|_| info!("Terminal closed"));
            };
            spawn_local(send.instrument(span.clone()));
        });
        self.on_data(&on_data);
        return on_data;
    }

    fn do_on_resize(&self, terminal: TerminalAddress) -> Closure<dyn FnMut(JsValue)> {
        let span = Span::current();
        let this = self.clone();
        let on_resize: Closure<dyn FnMut(JsValue)> = Closure::new(move |data: JsValue| {
            let _span = span.enter();
            let Some(data) = data.as_string() else {
                return;
            };
            #[derive(serde::Deserialize)]
            struct Dimensions {
                rows: i32,
                cols: i32,
            }
            let Ok(size) = serde_json::from_str::<Dimensions>(&data) else {
                return;
            };
            let Some(session) = this.session().as_string() else {
                return;
            };
            let Ok(attachment) = serde_json::from_str(&session) else {
                return;
            };
            let terminal = terminal.clone();
            spawn_local(async move {
                if let Err(error) = terminal_api::resize(
                    &terminal,
                    terminal_schema::Size {
                        rows: size.rows,
                        cols: size.cols,
                    },
                    attachment,
                )
                .await
                {
                    warn!("Failed to resize: {error}");
                }
            });
        });
        self.on_resize(&on_resize);
        on_resize
    }

    fn do_on_checkpoint(&self, terminal: TerminalAddress) -> Closure<dyn FnMut(JsValue)> {
        let this = self.clone();
        let callback = Closure::new(move |data: JsValue| {
            let Some(data) = data.as_string() else {
                return;
            };
            let Ok(checkpoint) = serde_json::from_str::<terminal_schema::TerminalCheckpoint>(&data)
            else {
                return;
            };
            let terminal = terminal.clone();
            let this = this.clone();
            spawn_local(async move {
                let session = serde_json::to_string(&checkpoint.attachment).unwrap_or_default();
                let result = crate::terminal::api::put_checkpoint(terminal, checkpoint).await;
                let success = result.is_ok();
                if let Err(error) = result {
                    warn!("Checkpoint upload failed: {error}");
                }
                if this.session().as_string().as_deref() == Some(session.as_str()) {
                    this.checkpoint_finished(success).await;
                }
            });
        });
        self.on_checkpoint(&callback);
        callback
    }

    fn do_on_title_change(&self, title: XSignal<TabTitle<XString>>) -> Closure<dyn FnMut(JsValue)> {
        let span = Span::current();
        let on_title_change: Closure<dyn FnMut(JsValue)> = Closure::new(move |data: JsValue| {
            let _span = span.enter();
            info!("Title changed: {data:?}");
            if let Some(new_title) = data.as_string() {
                title.update_mut(|t| TabTitle {
                    shell_title: new_title.into(),
                    override_title: t.override_title.take(),
                });
            }
        });
        self.on_title_change(&on_title_change);
        return on_title_change;
    }

    async fn stream_loop(
        &self,
        state: TerminalsState,
        terminal_def: TerminalDef,
        initialized: oneshot::Sender<()>,
        notify_mouse: watch::WatchRx,
    ) {
        let span = debug_span!("StreamLoop", terminal_address = %terminal_def.address);
        async {
            debug!("Start");
            let on_init = || {
                self.fit();
                let _ = initialized.send(());
                ready(())
            };
            let eos = terminal_api::stream(state, terminal_def, notify_mouse, on_init, |data| {
                self.send(data)
            })
            .await;
            match eos {
                Ok(()) => info!("End"),
                Err(error) => warn!("Failed: {error}"),
            }
        }
        .instrument(span)
        .await
    }
}

async fn write_loop(
    terminal: &TerminalAddress,
    xtermjs: &TerminalJs,
    input_rx: mpsc::UnboundedReceiver<terminal_schema::TerminalInput>,
    initialized: oneshot::Receiver<()>,
) {
    async {
        defer!(debug!("End"));
        debug!("Start");
        if initialized.await.is_err() {
            warn!("Terminal stream closed before initialization");
            return;
        }
        let mut input_rx = input_rx.ready_chunks(10);
        while let Some(data) = &input_rx.next().await {
            let data = data.clone();
            let Some(session) = xtermjs.session().as_string() else {
                continue;
            };
            let Ok(attachment) = serde_json::from_str(&session) else {
                continue;
            };
            if let Err(error) =
                crate::terminal::api::write(terminal.clone(), attachment, data).await
            {
                // Delivery is uncertain. Keep the terminal, but never replay input.
                error!("Failed to write to the terminal: {error}");
            }
        }
    }
    .instrument(debug_span!("WriteLoop"))
    .await
}
