#![cfg(not(feature = "diagnostics"))]
//! This module replaces the tracing crate with no-ops when tracing is disabled.
//!
//! This avoid building the tracing crate altogether in WASM when tracing is not needed.

mod span_macros;
mod trace_macros;

pub use crate::__diagnostics_debug as debug;
pub use crate::__diagnostics_debug_span as debug_span;
pub use crate::__diagnostics_error as error;
pub use crate::__diagnostics_error_span as error_span;
pub use crate::__diagnostics_info as info;
pub use crate::__diagnostics_info_span as info_span;
pub use crate::__diagnostics_trace as trace;
pub use crate::__diagnostics_trace_span as trace_span;
pub use crate::__diagnostics_warn as warn;
pub use crate::__diagnostics_warn_span as warn_span;

pub mod span {
    use std::marker::PhantomData;

    pub struct EnteredSpan(PhantomData<*mut ()>);

    #[derive(Clone)]
    pub struct Span;

    impl Span {
        pub fn enter(&self) -> EnteredSpan {
            EnteredSpan(PhantomData)
        }
        pub fn entered(&self) -> EnteredSpan {
            EnteredSpan(PhantomData)
        }
    }
}
