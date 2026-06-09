use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::Element;

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

    #[wasm_bindgen(js_name = "createSpeechRecognition")]
    pub fn create_speech_recognition(
        on_result: &Closure<dyn FnMut(JsValue)>,
        on_end: &Closure<dyn FnMut()>,
        on_error: &Closure<dyn FnMut(JsValue)>,
    ) -> JsValue;

    #[wasm_bindgen(js_name = "startSpeechRecognition")]
    pub fn start_speech_recognition(recognition: &JsValue);

    #[wasm_bindgen(js_name = "stopSpeechRecognition")]
    pub fn stop_speech_recognition(recognition: &JsValue);
}
