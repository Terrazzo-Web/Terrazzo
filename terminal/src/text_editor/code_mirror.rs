#![cfg(feature = "client")]

use terrazzo::prelude::Closure;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::Element;

pub struct CodeMirrorJs(CodeMirrorJsImpl);

impl Drop for CodeMirrorJs {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl std::ops::Deref for CodeMirrorJs {
    type Target = CodeMirrorJsImpl;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl CodeMirrorJs {
    pub fn new(
        element: Element,
        content: JsValue,
        onchange: &Closure<dyn FnMut(JsValue)>,
        base_path: String,
        full_path: String,
    ) -> Self {
        Self(CodeMirrorJsImpl::new(
            element, content, onchange, base_path, full_path,
        ))
    }
}

#[wasm_bindgen(module = "/src/text_editor/code_mirror.js")]
extern "C" {
    #[derive(Clone)]
    pub type CodeMirrorJsImpl;

    #[wasm_bindgen(constructor)]
    fn new(
        element: Element,
        content: JsValue,
        onchange: &Closure<dyn FnMut(JsValue)>,
        base_path: String,
        full_path: String,
    ) -> CodeMirrorJsImpl;

    #[wasm_bindgen(method)]
    fn destroy(this: &CodeMirrorJsImpl);

    #[wasm_bindgen(method)]
    pub fn set_content(this: &CodeMirrorJsImpl, content: String);

    #[wasm_bindgen(method)]
    pub fn cargo_check(this: &CodeMirrorJsImpl, diagnostics: JsValue);
}
