use terrazzo::owned_closure::XOwnedClosure;
use terrazzo::prelude::Ptr;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::Element;
use web_sys::js_sys::Function;

pub struct CodeMirrorJs {
    inner: CodeMirrorJsImpl,
    _onchange: Ptr<XOwnedClosure<dyn Fn(JsValue)>>,
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
        onchange: Ptr<XOwnedClosure<dyn Fn(JsValue)>>,
        base_path: String,
        full_path: String,
    ) -> Self {
        let inner = CodeMirrorJsImpl::new(
            element,
            original,
            content,
            onchange.as_function(),
            base_path,
            full_path,
        );
        Self {
            inner,
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
        onchange: Function,
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
