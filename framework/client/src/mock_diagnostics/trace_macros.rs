#[doc(hidden)]
#[macro_export]
macro_rules! __diagnostics_error {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info!($($arg)*)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __diagnostics_warn {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info!($($arg)*)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __diagnostics_info {
    ($($arg:tt)*) => {
        ()
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __diagnostics_debug {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info!($($arg)*)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __diagnostics_trace {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info!($($arg)*)
    };
}
