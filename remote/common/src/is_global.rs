/// Shortcut for global config types that must be thread-safe.
pub trait IsGlobal: Send + Sync + 'static {}

impl<C: Send + Sync + 'static> IsGlobal for C {}
