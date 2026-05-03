use std::sync::Arc;

use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::Element;
use web_sys::js_sys::Uint8Array;

terrazzo_css::import_style!(pub(super) style, "pdf_viewer.scss");

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
        Self(PdfJsImpl::new(element, decode_pdf(base64)))
    }

    pub fn set_content(&self, base64: String) {
        self.0.set_content(decode_pdf(&base64));
    }
}

fn decode_pdf(base64: &str) -> Uint8Array {
    let Ok(bytes) = BASE64_STANDARD.decode(base64) else {
        return Uint8Array::new_with_length(0);
    };
    let data = Uint8Array::new_with_length(bytes.len() as u32);
    data.copy_from(&bytes);
    data
}

#[wasm_bindgen(module = "/src/text_editor/ui/pdf_viewer.js")]
extern "C" {
    #[derive(Clone)]
    pub type PdfJsImpl;

    #[wasm_bindgen(constructor)]
    fn new(element: Element, data: Uint8Array) -> PdfJsImpl;

    #[wasm_bindgen(method)]
    fn destroy(this: &PdfJsImpl);

    #[wasm_bindgen(method)]
    pub fn set_content(this: &PdfJsImpl, data: Uint8Array);
}
