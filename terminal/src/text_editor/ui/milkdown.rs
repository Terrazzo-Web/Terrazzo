use terrazzo::prelude::Closure;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::Element;

use super::editor::EditorBody;

terrazzo_css::import_style!(pub(super) style, "milkdown.scss");

pub struct MilkdownJs {
    inner: MilkdownJsImpl,
    _onchange: Closure<dyn FnMut(JsValue)>,
    _oncursor: Closure<dyn FnMut(JsValue)>,
}

impl Drop for MilkdownJs {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl std::ops::Deref for MilkdownJs {
    type Target = MilkdownJsImpl;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl MilkdownJs {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        wysiwyg_pane: Element,
        source_pane: Element,
        original: JsValue,
        content: JsValue,
        onchange: Closure<dyn FnMut(JsValue)>,
        oncursor: Closure<dyn FnMut(JsValue)>,
        cursor_position: JsValue,
        base_path: String,
        full_path: String,
        focus_source: bool,
    ) -> Self {
        Self {
            inner: MilkdownJsImpl::new(
                wysiwyg_pane,
                source_pane,
                original,
                content,
                &onchange,
                &oncursor,
                cursor_position,
                base_path,
                full_path,
                focus_source,
            ),
            _onchange: onchange,
            _oncursor: oncursor,
        }
    }

    pub fn set_content(&self, content: String) {
        self.inner.set_content(content);
    }

    pub fn insert_text(&self, text: String) {
        self.inner.insert_text(text);
    }

    pub fn focus(&self) {
        self.inner.focus();
    }

    pub fn cargo_check(&self, diagnostics: JsValue) {
        self.inner.cargo_check(diagnostics);
    }
}

impl EditorBody for MilkdownJs {
    fn set_content(&self, content: String) {
        self.set_content(content);
    }

    fn insert_text(&self, text: String) {
        self.insert_text(text);
    }

    fn focus(&self) {
        self.focus();
    }

    fn cargo_check(&self, diagnostics: JsValue) {
        self.cargo_check(diagnostics);
    }
}

#[wasm_bindgen(module = "/src/text_editor/ui/milkdown.js")]
extern "C" {
    #[derive(Clone)]
    pub type MilkdownJsImpl;

    #[wasm_bindgen(constructor)]
    #[allow(clippy::too_many_arguments)]
    fn new(
        wysiwyg_pane: Element,
        source_pane: Element,
        original: JsValue,
        content: JsValue,
        onchange: &Closure<dyn FnMut(JsValue)>,
        oncursor: &Closure<dyn FnMut(JsValue)>,
        cursor_position: JsValue,
        base_path: String,
        full_path: String,
        focus_source: bool,
    ) -> MilkdownJsImpl;

    #[wasm_bindgen(method)]
    fn destroy(this: &MilkdownJsImpl);

    #[wasm_bindgen(method)]
    pub fn set_content(this: &MilkdownJsImpl, content: String);

    #[wasm_bindgen(method)]
    pub fn insert_text(this: &MilkdownJsImpl, text: String);

    #[wasm_bindgen(method)]
    pub fn focus(this: &MilkdownJsImpl);

    #[wasm_bindgen(method)]
    pub fn cargo_check(this: &MilkdownJsImpl, diagnostics: JsValue);
}
