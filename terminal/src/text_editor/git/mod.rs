use std::path::Path;
use std::sync::Arc;

use server_fn::Http;
use server_fn::ServerFnError;
use server_fn::codec::Json;
use terrazzo::server;

use crate::api::client_address::ClientAddress;
use crate::text_editor::fsio::FileMetadata;

#[server(protocol = Http<Json, Json>)]
pub async fn git_status(
    remote: ClientAddress,
    base: Arc<Path>,
) -> Result<Option<Vec<FileMetadata>>, ServerFnError> {
    service::git_status(remote, base).await
}

#[cfg(feature = "server")]
mod service;
