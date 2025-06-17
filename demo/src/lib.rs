#![cfg_attr(not(feature = "diagnostics"), allow(unused))]

mod api;
mod assets;
mod backend;
mod demo;
mod frontend;

#[cfg(feature = "server")]
pub use self::backend::run_server;
