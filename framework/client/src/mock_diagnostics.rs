#![cfg(not(feature = "diagnostics"))]
//! This module replaces the tracing crate with no-ops when tracing is disabled.
//!
//! This avoid building the tracing crate altogether in WASM when tracing is not needed.

mod span_macros;
mod trace_macros;

pub use crate::debug;
pub use crate::debug_span;
pub use crate::error;
pub use crate::error_span;
pub use crate::info;
pub use crate::info_span;
pub use crate::trace;
pub use crate::trace_span;
pub use crate::warn;
pub use crate::warn_span;

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
