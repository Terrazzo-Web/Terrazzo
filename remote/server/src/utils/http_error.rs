use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;

pub trait IsHttpError: std::error::Error + Sized {
    fn status_code(&self) -> StatusCode;
}

#[derive(thiserror::Error, Debug, Clone)]
#[error(transparent)]
pub struct HttpError<E>(#[from] E);

impl<E: IsHttpError> IntoResponse for HttpError<E> {
    fn into_response(self) -> Response {
        (self.0.status_code(), self.to_string()).into_response()
    }
}
