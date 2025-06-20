#[macro_export]
macro_rules! __diagnostics_error_span {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info_span!($($arg)*)
    }
}

#[macro_export]
macro_rules! __diagnostics_warn_span {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info_span!($($arg)*)
    }
}

#[macro_export]
macro_rules! __diagnostics_info_span {
    ($($args:tt)*) => {
        $crate::prelude::diagnostics::span::Span
    };
}

#[macro_export]
macro_rules! __diagnostics_debug_span {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info_span!($($arg)*)
    }
}

#[macro_export]
macro_rules! __diagnostics_trace_span {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info_span!($($arg)*)
    }
}
