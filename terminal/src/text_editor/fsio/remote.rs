#![cfg(feature = "server")]

use std::sync::Arc;

use super::File;
use crate::backend::client_service::grpc_error::GrpcError;
use crate::backend::client_service::remote_fn_service;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::side::SideViewList;

#[derive(Debug, serde::Serialize, serde:: Deserialize)]
pub struct LoadFileRequest {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "p"))]
    pub path: FilePath<Arc<str>>,
}

#[derive(Debug, serde::Serialize, serde:: Deserialize)]
pub struct ListFolderRequest {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "p"))]
    pub path: FilePath<Arc<str>>,
}

#[derive(Debug, serde::Serialize, serde:: Deserialize)]
pub struct FileExistsRequest {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "p"))]
    pub path: FilePath<Arc<str>>,
}

#[derive(Debug, serde::Serialize, serde:: Deserialize)]
pub struct PruneSideViewRequest {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "b"))]
    pub base: Arc<str>,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "t"))]
    pub tree: Arc<SideViewList>,
}

#[derive(Debug, serde::Serialize, serde:: Deserialize)]
pub struct StoreFileRequest {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "p"))]
    pub path: FilePath<Arc<str>>,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "c"))]
    pub content: String,
}

#[derive(Debug, serde::Serialize, serde:: Deserialize)]
pub struct CreateEntryRequest {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "p"))]
    pub path: FilePath<Arc<str>>,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "n"))]
    pub name: String,
}

#[derive(Debug, serde::Serialize, serde:: Deserialize)]
pub struct DeleteFileRequest {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "p"))]
    pub path: FilePath<Arc<str>>,
}

remote_fn_service::unary::declare_remote_fn!(
    LOAD_FILE_REMOTE_FN,
    super::LOAD_FILE,
    LoadFileRequest,
    Option<File>,
    |_server, arg: LoadFileRequest| async {
        let result = super::service::load_file(arg.path).await;
        result.map_err(GrpcError::from)
    }
);

remote_fn_service::unary::declare_remote_fn!(
    LIST_FOLDER_REMOTE_FN,
    super::LIST_FOLDER,
    ListFolderRequest,
    Option<Arc<Vec<super::FileMetadata>>>,
    |_server, arg: ListFolderRequest| async {
        let result = super::service::list_folder(arg.path).await;
        result.map_err(GrpcError::from)
    }
);

remote_fn_service::unary::declare_remote_fn!(
    FILE_EXISTS_REMOTE_FN,
    super::FILE_EXISTS,
    FileExistsRequest,
    bool,
    |_server, arg: FileExistsRequest| async {
        let result = super::service::file_exists(arg.path).await;
        result.map_err(GrpcError::from)
    }
);

remote_fn_service::unary::declare_remote_fn!(
    PRUNE_SIDE_VIEW_REMOTE_FN,
    super::PRUNE_SIDE_VIEW,
    PruneSideViewRequest,
    Option<Arc<SideViewList>>,
    |_server, arg: PruneSideViewRequest| async {
        let result = super::service::prune_side_view(arg.base, arg.tree).await;
        result.map_err(GrpcError::from)
    }
);

remote_fn_service::unary::declare_remote_fn!(
    CREATE_FILE_REMOTE_FN,
    super::CREATE_FILE,
    CreateEntryRequest,
    (),
    |_server, arg: CreateEntryRequest| async {
        let result = super::service::create_file(arg.path, arg.name).await;
        result.map_err(GrpcError::from)
    }
);

remote_fn_service::unary::declare_remote_fn!(
    CREATE_FOLDER_REMOTE_FN,
    super::CREATE_FOLDER,
    CreateEntryRequest,
    (),
    |_server, arg: CreateEntryRequest| async {
        let result = super::service::create_folder(arg.path, arg.name).await;
        result.map_err(GrpcError::from)
    }
);

remote_fn_service::unary::declare_remote_fn!(
    DELETE_FILE_REMOTE_FN,
    super::DELETE_FILE,
    DeleteFileRequest,
    (),
    |server, arg: DeleteFileRequest| {
        let server = server.clone();
        async move {
            let (trash, git_trash) = server
                .config()
                .server
                .with(|server| (server.trash.clone(), server.git_trash.clone()));
            let result = super::service::delete_file(arg.path, trash, git_trash).await;
            result.map_err(GrpcError::from)
        }
    }
);

remote_fn_service::unary::declare_remote_fn!(
    STORE_FILE_REMOTE_FN,
    super::STORE_FILE_IMPL,
    StoreFileRequest,
    (),
    |_server, arg: StoreFileRequest| async {
        let result = super::service::store_file(arg.path, arg.content).await;
        result.map_err(GrpcError::from)
    }
);
