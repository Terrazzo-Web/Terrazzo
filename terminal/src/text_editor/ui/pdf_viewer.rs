#![cfg(feature = "client")]

use std::sync::Arc;

use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::Element;

pub struct PdfJs(PdfJsImpl);

impl Drop for PdfJs {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl std::ops::Deref for PdfJs {
    type Target = PdfJsImpl;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PdfJs {
    pub fn new(element: Element, base64: &Arc<str>) -> Self {
        Self(PdfJsImpl::new(element, base64.to_string()))
    }

    pub fn set_content(&self, base64: String) {
        self.0.set_content(base64);
    }
}

#[wasm_bindgen(module = "/src/text_editor/ui/pdf_viewer.js")]
extern "C" {
    #[derive(Clone)]
    pub type PdfJsImpl;

    #[wasm_bindgen(constructor)]
    fn new(element: Element, base64: String) -> PdfJsImpl;

    #[wasm_bindgen(method)]
    fn destroy(this: &PdfJsImpl);

    #[wasm_bindgen(method)]
    pub fn set_content(this: &PdfJsImpl, base64: String);
}
