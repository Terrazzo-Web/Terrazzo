use std::path::Path;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Duration;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use server_fn::Http;
use server_fn::ServerFnError;
use server_fn::codec::Json;
use terrazzo::server;

use super::file_path::FilePath;
use super::side::SideViewNode;
use crate::api::client_address::ClientAddress;

#[cfg(feature = "server")]
pub mod api;
#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
mod fsmetadata;
#[cfg(feature = "server")]
mod git;
#[cfg(feature = "server")]
mod remote;
#[cfg(feature = "server")]
mod service;
#[cfg(feature = "client")]
pub mod ux;

pub static ROOT_BASE_PATH: LazyLock<Arc<Path>> = LazyLock::new(|| Path::new("/").into());
pub static ROOT_FILE_PATH: LazyLock<Arc<Path>> = LazyLock::new(|| Path::new("").into());

#[nameth]
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum File {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "D"))]
    TextFile {
        metadata: Arc<FileMetadata>,
        content: Arc<str>,
        original: Option<Arc<str>>,
    },
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "P"))]
    PdfFile {
        metadata: Arc<FileMetadata>,
        base64: Arc<str>,
    },
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "F"))]
    Folder(Arc<Vec<FileMetadata>>),
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "E"))]
    Error(String),
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FileMetadata {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "n"))]
    pub name: Arc<str>,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "s"))]
    pub size: Option<u64>,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "d"))]
    pub is_dir: bool,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "ct"))]
    pub created: Option<Duration>,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "at"))]
    pub accessed: Option<Duration>,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "mt"))]
    pub modified: Option<Duration>,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "m"))]
    pub mode: Option<u32>,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "u"))]
    pub user: Option<Arc<str>>,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "g"))]
    pub group: Option<Arc<str>>,
}

impl std::fmt::Debug for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_tuple(self.name());
        match self {
            Self::TextFile { content, .. } => f.field(&content.len()),
            Self::PdfFile { base64, .. } => f.field(&base64.len()),
            Self::Folder(folder) => f.field(&folder.len()),
            Self::Error(error) => f.field(error),
        }
        .finish()
    }
}

#[server(protocol = Http<Json, Json>)]
#[nameth]
async fn load_file(
    remote: ClientAddress,
    path: FilePath<Arc<Path>>,
) -> Result<Option<File>, ServerFnError> {
    Ok(remote::LOAD_FILE_REMOTE_FN
        .call(remote, remote::LoadFileRequest { path })
        .await?)
}

#[server(protocol = Http<Json, Json>)]
#[nameth]
async fn load_file_metadata(
    remote: ClientAddress,
    path: FilePath<Arc<Path>>,
) -> Result<Option<File>, ServerFnError> {
    Ok(remote::LOAD_FILE_METADATA_REMOTE_FN
        .call(remote, remote::LoadFileRequest { path })
        .await?)
}

#[server(protocol = Http<Json, Json>)]
#[nameth]
async fn list_folder(
    remote: ClientAddress,
    path: FilePath<Arc<Path>>,
) -> Result<Option<Arc<Vec<FileMetadata>>>, ServerFnError> {
    Ok(remote::LIST_FOLDER_REMOTE_FN
        .call(remote, remote::ListFolderRequest { path })
        .await?)
}

#[server(protocol = Http<Json, Json>)]
#[nameth]
async fn file_exists(
    remote: ClientAddress,
    path: FilePath<Arc<Path>>,
) -> Result<bool, ServerFnError> {
    Ok(remote::FILE_EXISTS_REMOTE_FN
        .call(remote, remote::FileExistsRequest { path })
        .await?)
}

#[server(protocol = Http<Json, Json>)]
#[nameth]
async fn prune_side_view(
    remote: ClientAddress,
    base: Arc<Path>,
    node: Arc<SideViewNode<()>>,
) -> Result<Option<Arc<SideViewNode<()>>>, ServerFnError> {
    Ok(remote::PRUNE_SIDE_VIEW_REMOTE_FN
        .call(remote, remote::PruneSideViewRequest { base, node })
        .await?)
}

#[server(protocol = Http<Json, Json>)]
#[nameth]
async fn create_file(
    remote: ClientAddress,
    path: FilePath<Arc<Path>>,
    name: String,
) -> Result<(), ServerFnError> {
    Ok(remote::CREATE_FILE_REMOTE_FN
        .call(remote, remote::CreateEntryRequest { path, name })
        .await?)
}

#[server(protocol = Http<Json, Json>)]
#[nameth]
async fn create_folder(
    remote: ClientAddress,
    path: FilePath<Arc<Path>>,
    name: String,
) -> Result<(), ServerFnError> {
    Ok(remote::CREATE_FOLDER_REMOTE_FN
        .call(remote, remote::CreateEntryRequest { path, name })
        .await?)
}

#[server(protocol = Http<Json, Json>)]
#[nameth]
async fn move_file(
    remote: ClientAddress,
    source: FilePath<Arc<Path>>,
    destination_folder: FilePath<Arc<Path>>,
) -> Result<(), ServerFnError> {
    Ok(remote::MOVE_FILE_REMOTE_FN
        .call(
            remote,
            remote::MoveFileRequest {
                source,
                destination_folder,
            },
        )
        .await?)
}

#[server(protocol = Http<Json, Json>)]
#[nameth]
async fn delete_file(
    remote: ClientAddress,
    path: FilePath<Arc<Path>>,
) -> Result<(), ServerFnError> {
    Ok(remote::DELETE_FILE_REMOTE_FN
        .call(remote, remote::DeleteFileRequest { path })
        .await?)
}

#[server(protocol = Http<Json, Json>)]
#[nameth]
async fn store_file_impl(
    remote: ClientAddress,
    path: FilePath<Arc<Path>>,
    content: String,
) -> Result<(), ServerFnError> {
    #[cfg(debug_assertions)]
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    Ok(remote::STORE_FILE_REMOTE_FN
        .call(remote, remote::StoreFileRequest { path, content })
        .await?)
}
