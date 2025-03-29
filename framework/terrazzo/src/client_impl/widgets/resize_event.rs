use std::cell::OnceCell;
use std::time::Duration;

use terrazzo_client::owned_closure;
use terrazzo_client::prelude::*;
use tracing::debug;
use wasm_bindgen::JsValue;
use web_sys::Window;

use super::debounce::DoDebounce as _;

/// Wraps a [XSignal<()>] that triggers when the window is resized.
///
/// The updates to the underlying signal are [debounced](super::debounce::DoDebounce).
pub struct ResizeEvent(OnceCell<XSignal<()>>);

impl ResizeEvent {
    /// A [XSignal<()>] that triggers when the window is resized.
    pub fn signal() -> &'static XSignal<()> {
        static SIGNAL: ResizeEvent = ResizeEvent(OnceCell::new());
        SIGNAL.0.get_or_init(|| XSignal::new("ResizeEvent", ()))
    }

    /// Configures [ResizeEvent].
    ///
    /// This method must be called once at page start-up time.
    pub fn set_up(window: &Window) {
        let closure = owned_closure::XOwnedClosure::new1(|self_drop| {
            Duration::from_millis(200)
                .with_max_delay()
                .debounce(move |resize_event: JsValue| {
                    let _self_drop = &self_drop;
                    debug!("Window resized: {resize_event:?}");
                    ResizeEvent::signal().force(())
                })
        });
        let function = closure.as_function().or_throw("as_function()");
        window
            .add_event_listener_with_callback("resize", &function)
            .or_throw("add_event_listener");
    }
}

unsafe impl Sync for ResizeEvent {}
