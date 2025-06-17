#[macro_export]
macro_rules! error_span {
    ($($arg:tt)*) => {
        $crate::tracing::info_span!($($arg)*)
    }
}

#[macro_export]
macro_rules! warn_span {
    ($($arg:tt)*) => {
        $crate::tracing::info_span!($($arg)*)
    }
}

#[macro_export]
macro_rules! info_span {
    ($lit:ident = $($rest:tt)*) => {
        $crate::tracing::info_span!($($rest)*)
    };
    (%$($rest:tt)*) => {
        $crate::tracing::info_span!($($rest)*)
    };
    (?$($rest:tt)*) => {
        $crate::tracing::info_span!($($rest)*)
    };
    ($expr:expr, $($rest:tt)*) => {{
        let _unused = &$expr;
        $crate::tracing::info_span!($($rest)*)
    }};
    ($expr:expr) => {{
        let _unused = &$expr;
        $crate::tracing::info_span!()
    }};
    () => {
        $crate::tracing::span::Span
    };
}

#[macro_export]
macro_rules! debug_span {
    ($($arg:tt)*) => {
        $crate::tracing::info_span!($($arg)*)
    }
}

#[macro_export]
macro_rules! trace_span {
    ($($arg:tt)*) => {
        $crate::tracing::info_span!($($arg)*)
    }
}
