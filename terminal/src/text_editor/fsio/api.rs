use std::path::PathBuf;

use terrazzo::axum::Router;
use terrazzo::axum::body::Bytes;
use terrazzo::axum::extract::Query;
use terrazzo::axum::response::IntoResponse;
use terrazzo::axum::routing::get;
use terrazzo::axum::routing::post;
use terrazzo::http::StatusCode;
use terrazzo::http::header;
use trz_gateway_common::dynamic_config::DynamicConfig;
use trz_gateway_common::dynamic_config::has_diff::DiffArc;
use trz_gateway_common::dynamic_config::mode;

use crate::backend::auth::AuthConfig;
use crate::backend::auth::layer::AuthLayer;
use crate::text_editor::file_path::FilePath;

pub(crate) fn fsio_routes(
    auth_config: &DiffArc<DynamicConfig<DiffArc<AuthConfig>, mode::RO>>,
) -> Router {
    Router::new()
        .nest(
            "/text_editor/fsio",
            Router::new()
                .route("/download", get(download_file))
                .route("/upload", post(upload_file)),
        )
        .route_layer(AuthLayer {
            auth_config: auth_config.clone(),
        })
}

async fn download_file(
    Query(path): Query<ApiFilePath>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let path = path.into_file_path().full_path();
    if !path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Path not found: {}", path.display()),
        ));
    }
    if !path.is_file() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Path is not a file: {}", path.display()),
        ));
    }

    let content = tokio::fs::read(path).await.map_err(internal_server_error)?;
    Ok((
        [(header::CONTENT_TYPE, "application/octet-stream")],
        content,
    ))
}

async fn upload_file(
    Query(path): Query<ApiFilePath>,
    content: Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let path = path.into_file_path().full_path();
    if path.is_dir() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Path is a directory: {}", path.display()),
        ));
    }
    if let Some(parent) = path.parent()
        && !parent.is_dir()
    {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Parent directory not found: {}", parent.display()),
        ));
    }

    tokio::fs::write(path, content)
        .await
        .map_err(internal_server_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn internal_server_error(error: std::io::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

#[derive(serde::Deserialize)]
struct ApiFilePath {
    base: PathBuf,
    file: PathBuf,
}

impl ApiFilePath {
    fn into_file_path(self) -> FilePath<PathBuf> {
        FilePath {
            base: self.base,
            file: self.file,
        }
    }
}
