use std::rc::Rc;

use scopeguard::ScopeGuard;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::Element;

#[derive(Clone)]
pub struct TerminalJsRc(Rc<ScopeGuard<TerminalJs, Box<dyn FnOnce(TerminalJs)>>>);

impl TerminalJsRc {
    pub fn new() -> Self {
        Self(Rc::new(scopeguard::guard(
            TerminalJs::new(),
            Box::new(|xtermjs| xtermjs.dispose()),
        )))
    }
}

impl std::ops::Deref for TerminalJsRc {
    type Target = TerminalJs;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[wasm_bindgen(module = "/src/terminal/javascript.js")]
extern "C" {
    #[derive(Clone)]
    pub type TerminalJs;

    #[wasm_bindgen(constructor)]
    pub fn new() -> TerminalJs;

    #[wasm_bindgen(method)]
    pub fn open(this: &TerminalJs, terminal_node: &Element);

    #[wasm_bindgen(method)]
    pub fn fit(this: &TerminalJs);

    #[wasm_bindgen(method)]
    pub fn focus(this: &TerminalJs);

    #[wasm_bindgen(method)]
    pub fn rows(this: &TerminalJs) -> JsValue;

    #[wasm_bindgen(method)]
    pub fn cols(this: &TerminalJs) -> JsValue;

    #[wasm_bindgen(method, js_name = "onData")]
    pub fn on_data(this: &TerminalJs, callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(method, js_name = "onResize")]
    pub fn on_resize(this: &TerminalJs, callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(method, js_name = "onTitleChange")]
    pub fn on_title_change(this: &TerminalJs, callback: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(method)]
    pub async fn send(this: &TerminalJs, data: JsValue);

    #[wasm_bindgen(method)]
    pub fn dispose(this: &TerminalJs);
}
