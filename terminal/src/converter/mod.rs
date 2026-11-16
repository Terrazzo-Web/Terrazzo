#![cfg(feature = "converter")]

use std::sync::Arc;

use crate::tiles::state::make_state;

mod api;
mod conversion_tabs;
mod service;
pub mod ui;

make_state!(content_state, Arc<str>);

#[cfg(all(feature = "server", feature = "text-editor"))]
pub(crate) use service::certificate_text_for_export as certificate_text;
