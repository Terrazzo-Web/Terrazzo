use std::convert::Infallible;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;

/// A trait implemented by errors that translate to HTTP status codes.
pub trait IsHttpError: std::error::Error + Sized {
    fn status_code(&self) -> StatusCode;
}

impl<T: IsHttpError> IsHttpError for Box<T> {
    fn status_code(&self) -> StatusCode {
        self.as_ref().status_code()
    }
}

/// A wrapper to translate errors into http [Response]s.
#[derive(thiserror::Error, Debug, Clone)]
#[error(transparent)]
pub struct HttpError<E: IsHttpError>(#[from] E);

impl<E: IsHttpError> IntoResponse for HttpError<E> {
    fn into_response(self) -> Response {
        (self.0.status_code(), self.to_string()).into_response()
    }
}

impl IsHttpError for Infallible {
    fn status_code(&self) -> StatusCode {
        unreachable!()
    }
}

impl IsHttpError for tonic::Status {
    fn status_code(&self) -> StatusCode {
        match self.code() {
            tonic::Code::Ok => StatusCode::OK,
            tonic::Code::Cancelled => StatusCode::from_u16(499).expect("Client Closed Request"),
            tonic::Code::Unknown => StatusCode::INTERNAL_SERVER_ERROR,
            tonic::Code::InvalidArgument => StatusCode::BAD_REQUEST,
            tonic::Code::DeadlineExceeded => StatusCode::GATEWAY_TIMEOUT,
            tonic::Code::NotFound => StatusCode::NOT_FOUND,
            tonic::Code::AlreadyExists => StatusCode::CONFLICT,
            tonic::Code::PermissionDenied => StatusCode::FORBIDDEN,
            tonic::Code::ResourceExhausted => StatusCode::TOO_MANY_REQUESTS,
            tonic::Code::FailedPrecondition => StatusCode::BAD_REQUEST,
            tonic::Code::Aborted => StatusCode::CONFLICT,
            tonic::Code::OutOfRange => StatusCode::BAD_REQUEST,
            tonic::Code::Unimplemented => StatusCode::NOT_IMPLEMENTED,
            tonic::Code::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            tonic::Code::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            tonic::Code::DataLoss => StatusCode::INTERNAL_SERVER_ERROR,
            tonic::Code::Unauthenticated => StatusCode::UNAUTHORIZED,
        }
    }
}
