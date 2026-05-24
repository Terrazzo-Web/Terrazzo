#![cfg(feature = "text-editor")]

mod autocomplete;
pub mod file_path;
mod fsio;
mod manager;
pub mod notify;
mod path_selector;
mod rust_lang;
mod search;
mod side;
mod state;
mod synchronized_state;
pub mod ui;

#[cfg(feature = "client")]
terrazzo_css::import_style!(style, "text_editor.scss");
