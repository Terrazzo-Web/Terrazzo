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
            debug!("Focus and fit size");
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
    let on_title_change = xtermjs.do_on_title_change(terminal_tab.title.clone());
    let selected = terminal_tab.selected.get_value_untracked();
    let io = async move {
        let (initialized_tx, initialized_rx) = oneshot::channel();
        let stream_loop = xtermjs.stream_loop(state, terminal_def, initialized_tx, notify_mouse);
        let write_loop = write_loop(&terminal_address, input_rx, initialized_rx);
        let unsubscribe_resize_event = ResizeEvent::signal().add_subscriber({
            let xtermjs = xtermjs.clone();
            move |_| xtermjs.fit()
        });
        if selected {
            xtermjs.focus();
        }
        // TODO: If write fails, we should not close the tab
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
    fn do_on_data(&self, input_tx: mpsc::UnboundedSender<String>) -> Closure<dyn FnMut(JsValue)> {
        let span = Span::current();
        let on_data: Closure<dyn FnMut(JsValue)> = Closure::new(move |data: JsValue| {
            let mut input_tx = input_tx.clone();
            let data = data.as_string().unwrap_or_default();
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
        let mut first_resize = true;
        let on_resize: Closure<dyn FnMut(JsValue)> = Closure::new(move |data| {
            let _span = span.enter();
            let first_resize = std::mem::replace(&mut first_resize, false);
            debug!("Resize: {data:?} first_resize:{first_resize}");
            let resize = this.clone().do_resize(terminal.clone(), first_resize);
            spawn_local(resize.in_current_span());
        });
        self.on_resize(&on_resize);
        return on_resize;
    }

    async fn do_resize(self, terminal: TerminalAddress, force: bool) {
        let size = terminal_schema::Size {
            rows: self.rows().as_f64().or_throw("rows") as i32,
            cols: self.cols().as_f64().or_throw("cols") as i32,
        };
        if let Err(error) = terminal_api::resize(&terminal, size, force).await {
            warn!("Failed to resize: {error}");
        }
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
    input_rx: mpsc::UnboundedReceiver<String>,
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
            let data = data.join("");
            if let Err(error) = terminal_api::write(terminal, data).await {
                error!("Failed to write to the terminal: {error}");
                return;
            }
        }
    }
    .instrument(debug_span!("WriteLoop"))
    .await
}
