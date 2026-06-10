#[cfg(feature = "server")]
mod event_handler;
#[cfg(feature = "client")]
pub mod manager;
pub mod server_fn;
#[cfg(feature = "server")]
pub mod service;
#[cfg(feature = "client")]
pub mod ui;
#[cfg(feature = "server")]
mod watcher;
