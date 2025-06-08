pub mod widgets;

pub use ::terrazzo_client::owned_closure;
pub use ::terrazzo_client::setup_logging;
pub use ::terrazzo_macro::*;

pub mod prelude {
    pub use ::terrazzo_client::prelude::*;
    pub type Prc<T> = crate::Prc<T>;
    pub type Pweak<T> = crate::Pweak<T>;
}
