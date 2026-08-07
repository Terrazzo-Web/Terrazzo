#![cfg(feature = "terminal")]

pub(crate) mod api;
#[cfg(feature = "client")]
mod attach;
#[cfg(feature = "client")]
pub(crate) mod client;
#[cfg(feature = "client")]
mod javascript;
#[cfg(feature = "server")]
mod service;
pub mod streams;
#[cfg(feature = "client")]
mod terminal_tab;
#[cfg(feature = "client")]
mod terminal_tabs;
#[cfg(feature = "client")]
pub mod ui;
