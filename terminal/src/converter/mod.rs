#![cfg(feature = "converter")]

use std::sync::Arc;

use crate::state::make_state::make_state;

pub(crate) mod api;
mod conversion_tabs;
pub(crate) mod service;
pub mod ui;

make_state!(content_state, Arc<str>);
