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
