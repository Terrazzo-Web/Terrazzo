pub trait IsGlobal: Send + Sync + 'static {}

impl<C: Send + Sync + 'static> IsGlobal for C {}
