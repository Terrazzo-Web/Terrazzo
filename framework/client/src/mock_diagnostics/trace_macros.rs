#[macro_export]
macro_rules! __diagnostics_error {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info!($($arg)*)
    };
}

#[macro_export]
macro_rules! __diagnostics_warn {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info!($($arg)*)
    };
}

#[macro_export]
macro_rules! __diagnostics_info {
    ($k:ident = $v:expr, $($arg:tt)*) => {{
        let _ = &$v;
        $crate::__diagnostics_info!($($arg)*)
    }};
    ($k:ident = ?$v:expr, $($arg:tt)*) => {{
        $crate::__diagnostics_info!($k = $v, $($arg)*)
    }};
    ($k:ident = %$v:expr, $($arg:tt)*) => {{
        $crate::__diagnostics_info!($k = $v, $($arg)*)
    }};

    ($k:ident, $($arg:tt)*) => {{
        let _ = &$k;
        $crate::__diagnostics_info!($($arg)*)
    }};
    (?$k:ident, $($arg:tt)*) => {{
        $crate::__diagnostics_info!($k, $($arg)*)
    }};
    (%$k:ident, $($arg:tt)*) => {{
        $crate::__diagnostics_info!($k, $($arg)*)
    }};

    ($fmt:literal, $($arg:tt)+) => {{
        let _ = format_args!($fmt, $($arg)+);
    }};
    ($fmt:literal,) => {{
        let _ = format_args!($fmt);
    }};
    ($fmt:literal) => {{
        let _ = format_args!($fmt);
    }};

    ($v:expr, $($arg:tt)*) => {{
        let _ = &$v;
        $crate::__diagnostics_info!($($arg)*)
    }};

    ($k:ident = $v:expr) => {{ let _ = &$v; }};
    ($k:ident = ?$v:expr) => {{ let _ = &$v; }};
    ($k:ident = %$v:expr) => {{ let _ = &$v; }};
    ($k:ident) => {{ let _ = &$k; }};
    (?$k:ident) => {{ let _ = &$k; }};
    (%$k:ident) => {{ let _ = &$k; }};
    ($v:expr) => {{ let _ = &$v; }};

    () => {{ () }};
}

#[macro_export]
macro_rules! __diagnostics_debug {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info!($($arg)*)
    };
}

#[macro_export]
macro_rules! __diagnostics_trace {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info!($($arg)*)
    };
}

#[macro_export]
macro_rules! __diagnostics_enabled {
    ($v:expr) => {{
        let _ = &$v;
        false
    }};
    ($($arg:tt)*) => {{
        false
    }};
}
