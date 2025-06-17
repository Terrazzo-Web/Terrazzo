#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info!($($arg)*)
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info!($($arg)*)
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        ()
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info!($($arg)*)
    };
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        $crate::prelude::diagnostics::info!($($arg)*)
    };
}
