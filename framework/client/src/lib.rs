#![cfg_attr(not(feature = "diagnostics"), allow(unused))]
#![doc = include_str!("../README.md")]

use debug_correlation_id::DebugCorrelationId;
use nameth::NamedType as _;

use self::owned_closure::XOwnedClosure;
use self::prelude::OrElseLog;

mod attribute;
mod debug_correlation_id;
mod element;
mod key;
mod mock_diagnostics;
mod node;
pub mod owned_closure;
pub mod prelude;
mod signal;
mod string;
mod template;
mod utils;

/// Configures tracing in the browser using [tracing_subscriber_wasm].
///
/// Run it once at page startup time.
#[cfg(feature = "diagnostics")]
pub fn setup_logging() {
    use tracing_subscriber_wasm::MakeConsoleWriter;

    tracing_subscriber::fmt()
        .with_max_level(crate::prelude::diagnostics::Level::TRACE)
        .with_writer(MakeConsoleWriter::default())
        .without_time()
        .with_ansi(false)
        .with_line_number(true)
        .with_file(true)
        .with_target(false)
        .init();
    let version = "1.0";
    crate::prelude::diagnostics::trace!(version, "Setting logging: TRACE");
    crate::prelude::diagnostics::debug!(version, "Setting logging: DEBUG");
    crate::prelude::diagnostics::info!(version, "Setting logging: INFO");
    crate::prelude::diagnostics::info!(
        "{}: {:?}",
        DebugCorrelationId::<&str>::type_name(),
        DebugCorrelationId::new(|| "here")
    );

    // Periodically clear the console
    if cfg!(feature = "concise-traces") {
        let closure = XOwnedClosure::new(|self_drop| {
            move || {
                let _self_drop = &self_drop;
                web_sys::console::clear();
            }
        });
        let window = web_sys::window().or_throw("window");
        window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                &closure.as_function(),
                /* 15 minutes */ 15 * 60 * 1000,
            )
            .or_throw("set_interval");
    }
}

#[cfg(not(feature = "diagnostics"))]
pub fn setup_logging() {}
