
mod autocomplete;
pub mod file_path;
pub mod fsio;
#[cfg(feature = "client")]
mod manager;
pub mod notify;
mod path_selector;
mod rust_lang;
mod search;
mod side;
mod state;
#[cfg(feature = "client")]
mod synchronized_state;
#[cfg(feature = "client")]
pub mod ui;

#[cfg(feature = "client")]
terrazzo_css::import_style!(style, "text_editor.scss");
