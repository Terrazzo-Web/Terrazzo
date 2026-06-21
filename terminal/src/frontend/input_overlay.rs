use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;

use terrazzo::autoclone;
use terrazzo::html;
use terrazzo::prelude::*;
use terrazzo::template;
use terrazzo::widgets::element_capture::ElementCapture;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlTextAreaElement;
use web_sys::KeyboardEvent;

use self::diagnostics::warn;
use crate::assets::icons;
use crate::frontend::speech_recognition;

terrazzo_css::import_style!(pub style, "input_overlay.scss");

struct SpeechRecognitionHandle {
    recognition: JsValue,
    _on_result: Closure<dyn FnMut(JsValue)>,
    _on_end: Closure<dyn FnMut()>,
    _on_error: Closure<dyn FnMut(JsValue)>,
}

pub static OPEN_COUNT: Mutex<i32> = Mutex::new(0);

#[derive(Debug, PartialEq, Eq)]
struct IsOpenToggle;

impl IsOpenToggle {
    fn new() -> Self {
        *OPEN_COUNT.lock().unwrap() += 1;
        Self
    }
}

impl Drop for IsOpenToggle {
    fn drop(&mut self) {
        *OPEN_COUNT.lock().unwrap() -= 1;
    }
}

#[html]
pub fn input_overlay(send: Ptr<dyn Fn(String)>, focus_target: Ptr<dyn Fn()>) -> XElement {
    let is_open = XSignal::new("input-overlay-open", false);
    let is_open_toggle = is_open.view("is-overlay-open-toggle", |t| {
        if *t { Some(IsOpenToggle::new()) } else { None }
    });
    let is_recording = XSignal::new("input-overlay-recording", false);
    let value = XSignal::new("input-overlay-value", XString::default());
    let textarea: ElementCapture<HtmlTextAreaElement> = Default::default();
    let speech_recognition: Rc<RefCell<Option<SpeechRecognitionHandle>>> = Default::default();
    let drop_is_open = scopeguard::guard(
        (is_recording.clone(), speech_recognition.clone()),
        |(is_recording, speech_recognition)| {
            if is_recording.get_value_untracked() {
                spawn_local(async move { stop_recording(is_recording, speech_recognition) });
            }
        },
    );

    div(
        before_render = move |_| {
            let _ = &drop_is_open;
            let _ = &is_open_toggle;
        },
        class = style::INPUT_OVERLAY,
        class %= overlay_class(is_open.clone()),
        #[cfg(not(feature = "client-prod"))]
        class = "input-overlay",
        div(
            class = style::INPUT_OVERLAY_BUTTONS,
            state_button(
                is_open.clone(),
                is_recording.clone(),
                value.clone(),
                textarea.clone(),
                speech_recognition.clone(),
            ),
            send_button(
                send.clone(),
                focus_target.clone(),
                is_open.clone(),
                is_recording.clone(),
                value.clone(),
                textarea.clone(),
                speech_recognition.clone(),
            ),
        ),
        compose_textarea(
            textarea.clone(),
            value.clone(),
            is_open.clone(),
            is_recording,
            send,
            focus_target,
            speech_recognition,
        ),
    )
}

#[template(wrap = true)]
fn overlay_class(#[signal] is_open: bool) -> XAttributeValue {
    is_open.then_some(style::ACTIVE)
}

#[autoclone]
#[html]
#[template(tag = textarea)]
fn compose_textarea(
    textarea: ElementCapture<HtmlTextAreaElement>,
    value: XSignal<XString>,
    is_open: XSignal<bool>,
    is_recording: XSignal<bool>,
    send: Ptr<dyn Fn(String)>,
    focus_target: Ptr<dyn Fn()>,
    speech_recognition: Rc<RefCell<Option<SpeechRecognitionHandle>>>,
) -> XElement {
    tag(
        before_render = textarea.capture(),
        class = style::INPUT_OVERLAY_TEXTAREA,
        #[cfg(not(feature = "client-prod"))]
        class = "input-overlay-textarea",
        input = move |_| {
            autoclone!(value, textarea);
            let new_value = textarea
                .try_with(|textarea| textarea.value())
                .unwrap_or_default();
            value.set(new_value);
        },
        keydown = move |event: KeyboardEvent| {
            autoclone!(
                send,
                focus_target,
                is_open,
                is_recording,
                value,
                textarea,
                speech_recognition
            );
            event.stop_propagation();
            match event.key().as_str() {
                "Escape" => {
                    stop_recording(is_recording.clone(), speech_recognition.clone());
                    is_open.set(false)
                }
                "Enter" if event.ctrl_key() || event.meta_key() => {
                    event.prevent_default();
                    send_value(
                        send.clone(),
                        focus_target.clone(),
                        MutableSignal::from(&is_open),
                        is_recording.clone(),
                        value.clone(),
                        textarea.clone(),
                        speech_recognition.clone(),
                    );
                }
                _ => {}
            }
        },
    )
}

#[html]
#[template(tag = img)]
fn send_button(
    send: Ptr<dyn Fn(String)>,
    focus_target: Ptr<dyn Fn()>,
    is_open: XSignal<bool>,
    is_recording: XSignal<bool>,
    value: XSignal<XString>,
    textarea: ElementCapture<HtmlTextAreaElement>,
    speech_recognition: Rc<RefCell<Option<SpeechRecognitionHandle>>>,
) -> XElement {
    return tag(
        class = style::INPUT_OVERLAY_SEND,
        class %= send_button_class(value.clone()),
        #[cfg(not(feature = "client-prod"))]
        class = "input-overlay-send",
        src = icons::send_fill(),
        title = "Send input",
        click = move |_| {
            send_value(
                send.clone(),
                focus_target.clone(),
                MutableSignal::from(&is_open),
                is_recording.clone(),
                value.clone(),
                textarea.clone(),
                speech_recognition.clone(),
            )
        },
    );

    #[template(wrap = true)]
    fn send_button_class(#[signal] value: XString) -> XAttributeValue {
        (!value.is_empty()).then_some(style::ACTIVE)
    }
}

fn send_value(
    send: Ptr<dyn Fn(String)>,
    focus_target: Ptr<dyn Fn()>,
    is_open: MutableSignal<bool>,
    is_recording: XSignal<bool>,
    value: XSignal<XString>,
    textarea: ElementCapture<HtmlTextAreaElement>,
    speech_recognition: Rc<RefCell<Option<SpeechRecognitionHandle>>>,
) {
    let data = textarea.try_with(|t| t.value()).unwrap_or_default();
    if data.is_empty() {
        return;
    }
    stop_recording(is_recording, speech_recognition);
    value.set(XString::default());
    textarea.try_with(|textarea| textarea.set_value(""));
    send(data);
    is_open.set(false);
    focus_target();
}

#[autoclone]
#[html]
fn state_button(
    is_open: XSignal<bool>,
    is_recording: XSignal<bool>,
    value: XSignal<XString>,
    textarea: ElementCapture<HtmlTextAreaElement>,
    speech_recognition: Rc<RefCell<Option<SpeechRecognitionHandle>>>,
) -> XElement {
    img(
        class = style::INPUT_OVERLAY_BUTTON,
        class %= state_button_class(is_open.clone(), is_recording.clone()),
        #[cfg(not(feature = "client-prod"))]
        class = "input-overlay-button",
        src %= state_button_icon(is_open.clone(), is_recording.clone()),
        title %= state_button_title(is_open.clone(), is_recording.clone()),
        click = move |_| {
            autoclone!(is_open, is_recording, value, textarea, speech_recognition);
            let textarea_hidden = textarea
                .try_with(|textarea| textarea.offset_parent().is_none())
                .unwrap_or(true);
            if !is_open.get_value_untracked() || textarea_hidden {
                is_open.force(true);
                if let Some(Err(error)) = textarea.try_with(|textarea| textarea.focus()) {
                    warn!("Failed to focus input overlay: {error:?}");
                }
                return;
            }
            if is_recording.get_value_untracked() {
                stop_recording(is_recording.clone(), speech_recognition.clone());
            } else {
                start_recording(
                    is_recording.clone(),
                    value.clone(),
                    textarea.clone(),
                    speech_recognition.clone(),
                );
            }
        },
    )
}

#[template(wrap = true)]
fn state_button_class(#[signal] is_open: bool, #[signal] is_recording: bool) -> XAttributeValue {
    (is_open || is_recording).then_some(style::ACTIVE)
}

#[template(wrap = true)]
fn state_button_icon(#[signal] is_open: bool, #[signal] is_recording: bool) -> XAttributeValue {
    if is_recording {
        icons::mic_fill()
    } else if is_open {
        icons::mic_mute_fill()
    } else {
        icons::paragraph()
    }
}

#[template(wrap = true)]
fn state_button_title(#[signal] is_open: bool, #[signal] is_recording: bool) -> XAttributeValue {
    if is_recording {
        "Stop dictation"
    } else if is_open {
        "Start dictation"
    } else {
        "Compose input"
    }
}

fn start_recording(
    is_recording: XSignal<bool>,
    value: XSignal<XString>,
    textarea: ElementCapture<HtmlTextAreaElement>,
    speech_recognition: Rc<RefCell<Option<SpeechRecognitionHandle>>>,
) {
    let original_value = value.get_value_untracked();
    let on_result: Closure<dyn FnMut(JsValue)> = Closure::new({
        let value = value.clone();
        let textarea = textarea.clone();
        move |transcript: JsValue| {
            let transcript = transcript.as_string().unwrap_or_default();
            let mut new_value = original_value.to_string();
            if new_value.is_empty() || new_value.ends_with(char::is_whitespace) {
                new_value += &transcript;
            } else {
                new_value += &format!(" {}", transcript);
            }
            value.set(new_value.clone());
            textarea.try_with(|textarea| textarea.set_value(&new_value));
        }
    });
    let on_end: Closure<dyn FnMut()> = Closure::new({
        let is_recording = is_recording.clone();
        move || is_recording.set(false)
    });
    let on_error: Closure<dyn FnMut(JsValue)> = Closure::new({
        let is_recording = is_recording.clone();
        move |error: JsValue| {
            warn!("Speech recognition failed: {error:?}");
            is_recording.set(false);
        }
    });
    let recognition = speech_recognition::create_speech_recognition(&on_result, &on_end, &on_error);
    if recognition.is_null() || recognition.is_undefined() {
        warn!("Speech recognition is not supported in this browser");
        return;
    }
    speech_recognition::start_speech_recognition(&recognition);
    is_recording.set(true);
    *speech_recognition.borrow_mut() = Some(SpeechRecognitionHandle {
        recognition,
        _on_result: on_result,
        _on_end: on_end,
        _on_error: on_error,
    });
}

fn stop_recording(
    is_recording: XSignal<bool>,
    speech_recognition: Rc<RefCell<Option<SpeechRecognitionHandle>>>,
) {
    if let Some(speech_recognition) = speech_recognition.borrow().as_ref() {
        speech_recognition::stop_speech_recognition(&speech_recognition.recognition);
    }
    is_recording.set(false);
}
