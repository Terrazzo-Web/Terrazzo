mod api;
mod assets;
#[cfg(feature = "server")]
mod backend;
#[cfg(feature = "converter")]
mod converter;
#[cfg(feature = "client")]
mod frontend;
#[cfg(feature = "logs-panel")]
mod logs;
#[cfg(feature = "port-forward")]
mod portforward;
#[cfg(feature = "server")]
#[cfg(feature = "terminal")]
mod processes;
#[cfg(feature = "terminal")]
mod terminal;
mod terminal_id;
#[cfg(feature = "text-editor")]
mod text_editor;
mod tiles;
mod utils;

#[cfg(test)]
use fluent_asserter as _;
#[cfg(test)]
use tempfile as _;

#[cfg(feature = "server")]
pub use self::backend::RunServerError;
#[cfg(feature = "server")]
pub use self::backend::run_server;
