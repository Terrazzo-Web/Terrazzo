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
    ($name:literal, $($arg:tt)*) => {{
        $crate::__diagnostics_info_span!($($arg)*)
    }};
    ($name:literal) => {{
        $crate::prelude::diagnostics::span::Span
    }};

    ($k:ident = $v:expr, $($arg:tt)*) => {{
        let _ = &$v;
        $crate::__diagnostics_info_span!($($arg)*)
    }};
    ($k:ident = ?$v:expr, $($arg:tt)*) => {{
        $crate::__diagnostics_info_span!($k = $v, $($arg)*)
    }};
    ($k:ident = %$v:expr, $($arg:tt)*) => {{
        $crate::__diagnostics_info_span!($k = $v, $($arg)*)
    }};

    ($k:ident, $($arg:tt)*) => {{
        let _ = &$k;
        $crate::__diagnostics_info_span!($($arg)*)
    }};
    (?$k:ident, $($arg:tt)*) => {{
        $crate::__diagnostics_info_span!($k, $($arg)*)
    }};
    (%$k:ident, $($arg:tt)*) => {{
        $crate::__diagnostics_info_span!($k, $($arg)*)
    }};

    ($k:ident = $v:expr) => {{
        let _ = &$v;
        $crate::prelude::diagnostics::span::Span
    }};
    ($k:ident = ?$v:expr) => {{
        let _ = &$v;
        $crate::prelude::diagnostics::span::Span
    }};
    ($k:ident = %$v:expr) => {{
        let _ = &$v;
        $crate::prelude::diagnostics::span::Span
    }};
    ($k:ident) => {{
        let _ = &$k;
        $crate::prelude::diagnostics::span::Span
    }};
    (?$k:ident) => {{
        let _ = &$k;
        $crate::prelude::diagnostics::span::Span
    }};
    (%$k:ident) => {{
        let _ = &$k;
        $crate::prelude::diagnostics::span::Span
    }};
    () => {{
        $crate::prelude::diagnostics::span::Span
    }};
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
