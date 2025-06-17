#![cfg(feature = "client")]

use std::sync::Mutex;

use diagnostics::info;
use terrazzo::prelude::*;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::demo;

#[wasm_bindgen]
pub fn start() {
    terrazzo::setup_logging();
    info!("Starting client");

    let window = web_sys::window().or_throw("window");
    let document = window.document().or_throw("document");
    let main = document
        .get_element_by_id("main")
        .or_throw("#main not found");

    let template = XTemplate::new(Ptr::new(Mutex::new(main.clone())));
    std::mem::forget(template.clone());
    let consumer = demo::run(template);
    std::mem::forget(consumer);
}
