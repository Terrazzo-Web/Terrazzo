pub trait IsConfiguration: Send + Sync + 'static {}

impl<C: Send + Sync + 'static> IsConfiguration for C {}
