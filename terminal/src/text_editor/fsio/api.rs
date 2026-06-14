use std::path::PathBuf;

use terrazzo::axum::Router;
use terrazzo::axum::body::Body;
use terrazzo::axum::extract::Query;
use terrazzo::axum::response::IntoResponse;
use terrazzo::axum::routing::get;
use terrazzo::axum::routing::post;
use terrazzo::http::StatusCode;
use terrazzo::http::header;
use tokio::io::AsyncWriteExt as _;
use tokio_stream::StreamExt as _;
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
            /* TODO

            See how terminal API sends the ClientAddress as part of the request.

                    pub async fn set_title(
                        server: Arc<Server>,
                        Json(request): Json<SetTitleRequest>,
                    ) -> Result<(), HttpError<self::terminal_service::set_title::SetTitleError>> {
                        let client_address = request.terminal.via.to_vec();
                        Ok(self::terminal_service::set_title::set_title(
                            &server,
                            &client_address,
                            SetTitleRequestProto {
                                address: Some(request.terminal.into()),
                                shell_title: request.title.shell_title,
                                override_title: request.title.override_title.map(|s| MaybeString { s }),
                            },
                        )
                        .await?)
                    }

            I want the download and upload API to behave the same: the file should be uploaded and downloaded from potentially a remote, not from the local server.

            You need to create a new gRPC API in terminal/src/backend/client_service/text_editor_service.
            Follow example terminal/src/backend/client_service/notify_service

            The goal is the

            */
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
    content: Body,
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

    let mut file = tokio::fs::File::create(path)
        .await
        .map_err(internal_server_error)?;
    let mut content = content.into_data_stream();
    while let Some(chunk) = content.next().await {
        let chunk = chunk.map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
        file.write_all(&chunk)
            .await
            .map_err(internal_server_error)?;
    }

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
