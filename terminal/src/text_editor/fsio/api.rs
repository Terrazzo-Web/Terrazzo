use std::path::PathBuf;
use std::sync::Arc;

use terrazzo::axum::Router;
use terrazzo::axum::body::Body;
use terrazzo::axum::extract::Query;
use terrazzo::axum::extract::State;
use terrazzo::axum::response::IntoResponse;
use terrazzo::axum::routing::get;
use terrazzo::axum::routing::post;
use terrazzo::http::StatusCode;
use terrazzo::http::header;
use tokio_stream::StreamExt as _;
use trz_gateway_common::dynamic_config::DynamicConfig;
use trz_gateway_common::dynamic_config::has_diff::DiffArc;
use trz_gateway_common::dynamic_config::mode;
use trz_gateway_common::http_error::IsHttpError as _;
use trz_gateway_common::id::ClientName;

use crate::api::client_address::ClientAddress;
use crate::backend::Server;
use crate::backend::auth::AuthConfig;
use crate::backend::auth::layer::AuthLayer;
use crate::backend::client_service::text_editor_service;
use crate::text_editor::file_path::FilePath;

pub(crate) fn fsio_routes(
    auth_config: &DiffArc<DynamicConfig<DiffArc<AuthConfig>, mode::RO>>,
    server: &Arc<Server>,
) -> Router {
    Router::new()
        .nest(
            "/text_editor/fsio",
            Router::new()
                .route("/download", get(download_file))
                .route("/upload", post(upload_file))
                .with_state(server.clone()),
        )
        .route_layer(AuthLayer {
            auth_config: auth_config.clone(),
        })
}

async fn download_file(
    Query(path): Query<ApiFilePath>,
    State(server): State<Arc<Server>>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let (remote, path) = path.into_remote_and_file_path();
    let content = text_editor_service::download(&server, &remote, path)
        .await
        .map_err(api_error)?;
    let content = content.map(|chunk| {
        chunk
            .inspect_err(text_editor_service::warn_stream_error)
            .map_err(|error| std::io::Error::other(error.to_string()))
    });
    Ok((
        [(header::CONTENT_TYPE, "application/octet-stream")],
        Body::from_stream(content),
    ))
}

async fn upload_file(
    Query(path): Query<ApiFilePath>,
    State(server): State<Arc<Server>>,
    content: Body,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let (remote, path) = path.into_remote_and_file_path();
    let content = content.into_data_stream().map(|chunk| {
        chunk.map_err(|error| {
            text_editor_service::TextEditorFsioError::IO(std::io::Error::other(error.to_string()))
        })
    });
    text_editor_service::upload(&server, &remote, path, content)
        .await
        .map_err(api_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn api_error(error: text_editor_service::TextEditorFsioError) -> (StatusCode, String) {
    (error.status_code(), error.to_string())
}

#[derive(serde::Deserialize)]
struct ApiFilePath {
    base: PathBuf,
    file: PathBuf,
    #[serde(default, deserialize_with = "deserialize_remote")]
    remote: ClientAddress,
}

impl ApiFilePath {
    fn into_remote_and_file_path(self) -> (Vec<ClientName>, FilePath<PathBuf>) {
        (
            self.remote.to_vec(),
            FilePath {
                base: self.base,
                file: self.file,
            },
        )
    }
}

fn deserialize_remote<'de, D>(deserializer: D) -> Result<ClientAddress, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let remote = <Option<String> as serde::Deserialize>::deserialize(deserializer)?;
    let Some(remote) = remote.filter(|remote| !remote.is_empty()) else {
        return Ok(ClientAddress::default());
    };
    serde_json::from_str(&remote).map_err(serde::de::Error::custom)
}
