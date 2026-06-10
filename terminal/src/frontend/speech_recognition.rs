use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(module = "/src/frontend/speech_recognition.js")]
extern "C" {
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
