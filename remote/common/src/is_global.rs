/// Shortcut for global config types that must be thread-safe.
pub trait IsGlobal: Send + Sync + 'static {}

impl<C: Send + Sync + 'static> IsGlobal for C {}

/// Shortcut for [error](std::error::Error) types that must be thread-safe.
pub trait IsGlobalError: IsGlobal + std::error::Error {}

impl<E: IsGlobal + std::error::Error> IsGlobalError for E {}
