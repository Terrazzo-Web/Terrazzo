use named::named;
use named::NamedEnumValues as _;
use terrazzo::prelude::OrElseLog;
use tracing::warn;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::Headers;
use web_sys::Request;
use web_sys::RequestInit;
use web_sys::RequestMode;
use web_sys::Response;

pub mod mult;

const BASE_URL: &str = "/api";
const APPLICATION_JSON: &str = "application/json";

async fn send_request(
    method: Method,
    url: String,
    on_request: impl FnOnce(&RequestInit),
) -> Result<Response, SendRequestError> {
    let request = RequestInit::new();
    request.set_method(method.name());
    request.set_mode(RequestMode::SameOrigin);
    on_request(&request);
    let request = Request::new_with_str_and_init(&url, &request);
    let request = request.map_err(|error| SendRequestError::InvalidUrl { url, error })?;
    let window = web_sys::window().or_throw("window");
    let promise = window.fetch_with_request(&request);
    let response = JsFuture::from(promise)
        .await
        .map_err(|error| SendRequestError::RequestError { error })?;
    let response: Response = response
        .dyn_into()
        .map_err(|error| SendRequestError::UnexpectedResponseObject { error })?;
    if !response.ok() {
        warn!("Request failed: {}", response.status());
        return Err(SendRequestError::RequestFailed {
            code: response.status(),
            message: response.status_text(),
        });
    }
    return Ok(response);
}

#[named]
#[derive(Clone, Copy)]
#[allow(clippy::upper_case_acronyms)]
enum Method {
    POST,
}

#[named]
#[derive(thiserror::Error, Debug)]
pub enum SendRequestError {
    #[error("[{}] Invalid url='{url}': {error:?}", self.name())]
    InvalidUrl { url: String, error: JsValue },

    #[error("[{}] {error:?}", self.name())]
    RequestError { error: JsValue },

    #[error("[{}] Unexpected {error:?}", self.name())]
    UnexpectedResponseObject { error: JsValue },

    #[error("[{}][{code}] {message}", self.name())]
    RequestFailed { code: u16, message: String },
}

fn set_content_type_json(headers: &mut Headers) {
    headers
        .set("content-type", APPLICATION_JSON)
        .or_throw("Set 'content-type'");
}
