#![cfg(feature = "terminal")]

mod api;
#[cfg(feature = "client")]
mod attach;
#[cfg(feature = "client")]
mod input_overlay;
#[cfg(feature = "client")]
mod javascript;
#[cfg(feature = "server")]
mod service;
#[cfg(feature = "client")]
mod speech_recognition;
#[cfg(feature = "client")]
mod terminal_tab;
#[cfg(feature = "client")]
mod terminal_tabs;
#[cfg(feature = "client")]
pub mod ui;
