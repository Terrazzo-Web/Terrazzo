
use std::sync::Arc;

use crate::tiles::state::make_state;

mod api;
#[cfg(feature = "client")]
mod conversion_tabs;
#[cfg(feature = "server")]
mod service;
#[cfg(feature = "client")]
pub mod ui;

make_state!(content_state, Arc<str>);
