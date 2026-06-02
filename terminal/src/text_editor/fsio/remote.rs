#![cfg(feature = "server")]

use std::future::ready;
use std::sync::Arc;

use super::File;
use crate::backend::client_service::grpc_error::GrpcError;
use crate::backend::client_service::remote_fn_service;
use crate::text_editor::file_path::FilePath;

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
    |_server, arg: LoadFileRequest| {
        let result = super::service::load_file(arg.path);
        ready(result.map_err(GrpcError::from))
    }
);

remote_fn_service::unary::declare_remote_fn!(
    LIST_FOLDER_REMOTE_FN,
    super::LIST_FOLDER,
    ListFolderRequest,
    Option<Arc<Vec<super::FileMetadata>>>,
    |_server, arg: ListFolderRequest| {
        let result = super::service::list_folder(arg.path);
        ready(result.map_err(GrpcError::from))
    }
);

remote_fn_service::unary::declare_remote_fn!(
    CREATE_FILE_REMOTE_FN,
    super::CREATE_FILE,
    CreateEntryRequest,
    (),
    |_server, arg: CreateEntryRequest| {
        let result = super::service::create_file(arg.path, arg.name);
        ready(result.map_err(GrpcError::from))
    }
);

remote_fn_service::unary::declare_remote_fn!(
    CREATE_FOLDER_REMOTE_FN,
    super::CREATE_FOLDER,
    CreateEntryRequest,
    (),
    |_server, arg: CreateEntryRequest| {
        let result = super::service::create_folder(arg.path, arg.name);
        ready(result.map_err(GrpcError::from))
    }
);

remote_fn_service::unary::declare_remote_fn!(
    DELETE_FILE_REMOTE_FN,
    super::DELETE_FILE,
    DeleteFileRequest,
    (),
    |server, arg: DeleteFileRequest| {
        let trash = server.config().server.with(|server| server.trash.clone());
        let result = super::service::delete_file(arg.path, trash);
        ready(result.map_err(GrpcError::from))
    }
);

remote_fn_service::unary::declare_remote_fn!(
    STORE_FILE_REMOTE_FN,
    super::STORE_FILE_IMPL,
    StoreFileRequest,
    (),
    |_server, arg: StoreFileRequest| {
        let result = super::service::store_file(arg.path, arg.content);
        ready(result.map_err(GrpcError::from))
    }
);
