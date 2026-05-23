use terrazzo::prelude::Closure;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::Element;

pub struct CodeMirrorJs {
    inner: CodeMirrorJsImpl,
    _onchange: Closure<dyn FnMut(JsValue)>,
}

impl Drop for CodeMirrorJs {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl std::ops::Deref for CodeMirrorJs {
    type Target = CodeMirrorJsImpl;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl CodeMirrorJs {
    pub fn new(
        element: Element,
        original: JsValue,
        content: JsValue,
        onchange: Closure<dyn FnMut(JsValue)>,
        base_path: String,
        full_path: String,
    ) -> Self {
        Self {
            inner: CodeMirrorJsImpl::new(
                element, original, content, &onchange, base_path, full_path,
            ),
            _onchange: onchange,
        }
    }

    pub fn set_content(&self, content: String) {
        self.inner.set_content(content);
    }

    pub fn cargo_check(&self, diagnostics: JsValue) {
        self.inner.cargo_check(diagnostics);
    }
}

#[wasm_bindgen(module = "/src/text_editor/ui/code_mirror.js")]
extern "C" {
    #[derive(Clone)]
    pub type CodeMirrorJsImpl;

    #[wasm_bindgen(constructor)]
    fn new(
        element: Element,
        original: JsValue,
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
