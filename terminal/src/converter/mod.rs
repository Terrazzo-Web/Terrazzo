#![cfg(feature = "converter")]

use std::sync::Arc;

use crate::tiles::state::make_state;

mod api;
mod conversion_tabs;
mod service;
pub mod ui;

make_state!(content_state, Arc<str>);
