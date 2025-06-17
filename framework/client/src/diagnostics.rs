#![allow(unused)]

mod trace_macros;

mod span_macros;

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
