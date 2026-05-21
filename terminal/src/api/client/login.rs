use nameth::NamedEnumValues as _;
use nameth::nameth;
use wasm_bindgen_futures::JsFuture;
use web_sys::Response;

use super::request::Method;
use super::request::SendRequestError;
use super::request::send_request;
use super::request::set_json_body;

#[nameth]
pub async fn login(password: Option<&str>) -> Result<(), LoginError> {
    let response: Response = send_request(
        Method::POST,
        format!("/api/{LOGIN} "),
        set_json_body(&password)?,
    )
    .await?;
    let message = response.text().map_err(|_| LoginError::MissingBody)?;
    let message = JsFuture::from(message)
        .await
        .map_err(|_| LoginError::FailedBody)?;
    let message = message.as_string().ok_or(LoginError::InvalidBody)?;
    if message == "LOGIN_REQUIRED" {
        return Err(LoginError::LoginRequired);
    }
    Ok(())
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum LoginError {
    #[error("[{n}] {0}", n = self.name())]
    SendRequestError(#[from] SendRequestError),

    #[error("[{n}] {0}", n = self.name())]
    InvalidJson(#[from] serde_json::Error),

    #[error("[{n}] Authentication is required", n = self.name())]
    LoginRequired,

    #[error("[{n}] Missing response body", n = self.name())]
    MissingBody,

    #[error("[{n}] Failed response body", n = self.name())]
    FailedBody,

    #[error("[{n}] Invalid response body", n = self.name())]
    InvalidBody,
}
