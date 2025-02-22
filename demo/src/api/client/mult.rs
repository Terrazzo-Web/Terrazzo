use nameth::NamedEnumValues as _;
use nameth::nameth;
use terrazzo::prelude::OrElseLog as _;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::Headers;
use web_sys::Response;

use super::BASE_URL;
use super::Method;
use super::SendRequestError;
use super::send_request;
use super::set_content_type_json;

#[nameth]
pub async fn mult(a: i32, b: i32) -> Result<i32, MultError> {
    let data = serde_json::to_string(&(a, b))?;
    let response: Response =
        send_request(Method::POST, format!("{BASE_URL}/{MULT}"), move |request| {
            let mut headers = Headers::new().or_throw("Headers::new()");
            set_content_type_json(&mut headers);
            request.set_headers(headers.as_ref());
            request.set_body(&JsValue::from_str(&data));
        })
        .await?;
    let result = response
        .text()
        .map_err(|_| MultError::MissingResponseBody)?;
    let result = JsFuture::from(result)
        .await
        .map_err(|_| MultError::FailedResponseBody)?;
    let body = result.as_string().ok_or(MultError::InvalidUtf8)?;
    Ok(serde_json::from_str(&body)?)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MultError {
    #[error("[{n}] {0}", n = self.name())]
    SendRequestError(#[from] SendRequestError),

    #[error("[{n}] Missing response body", n = self.name())]
    MissingResponseBody,

    #[error("[{n}] Failed to download the response body", n = self.name())]
    FailedResponseBody,

    #[error("[{n}] The response body is not a valid UTF-8 string", n = self.name())]
    InvalidUtf8,

    #[error("[{n}] {0}", n = self.name())]
    InvalidJson(#[from] serde_json::Error),
}
