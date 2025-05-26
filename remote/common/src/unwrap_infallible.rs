//! Utils for [Infallible].

use core::convert::Infallible;

/// Helper to unwrap [Result<_, Infallible>].
///
/// Such errors are always safe to unwrap without checking.
pub trait UnwrapInfallible {
    type Ok;
    fn unwrap_infallible(self) -> Self::Ok;
}

impl<T> UnwrapInfallible for Result<T, Infallible> {
    type Ok = T;
    fn unwrap_infallible(self) -> T {
        self.unwrap_or_else(|never| match never {})
    }
}
