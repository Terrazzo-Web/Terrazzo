#![doc = include_str!("../README.md")]

pub use ::autoclone::autoclone;

#[cfg(feature = "client")]
mod client_impl;
#[cfg(feature = "client")]
pub use self::client_impl::*;

#[cfg(feature = "server")]
mod server_impl;
#[cfg(feature = "server")]
pub use self::server_impl::*;

#[macro_export]
macro_rules! declare_trait_aliias {
    ($name_alias:ident, $($trait:tt)*) => {
        pub trait $name_alias: $($trait)+ {}
        impl<T: $($trait)+> $name_alias for T {}
    };
}
