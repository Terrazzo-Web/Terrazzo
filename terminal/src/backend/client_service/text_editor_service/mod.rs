mod callback;
mod grpc;

use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use futures::StreamExt as _;
use futures::stream;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use prost::bytes::Bytes;
use terrazzo::http::StatusCode;
use tokio::io::AsyncReadExt as _;
use tokio::io::AsyncWriteExt as _;
use tonic::Code;
use tracing::warn;
use trz_gateway_common::http_error::IsHttpError;
use trz_gateway_common::id::ClientName;

use crate::backend::Server;
use crate::backend::client_service::grpc_error::IsGrpcError;
use crate::backend::protos::terrazzo::texteditor::FilePath as FilePathProto;
use crate::text_editor::file_path::FilePath;

pub(super) const DOWNLOAD_CHUNK_SIZE: usize = 64 * 1024;

pub type DownloadStream =
    Pin<Box<dyn Stream<Item = Result<Bytes, TextEditorFsioError>> + Send + 'static>>;

pub async fn download(
    server: &Arc<Server>,
    client_address: &[impl AsRef<str>],
    path: FilePath<PathBuf>,
) -> Result<DownloadStream, TextEditorFsioError> {
    callback::DownloadCallback::download(server, client_address, path).await
}

pub async fn upload<S>(
    server: &Arc<Server>,
    client_address: &[impl AsRef<str>],
    path: FilePath<PathBuf>,
    content: S,
) -> Result<(), TextEditorFsioError>
where
    S: Stream<Item = Result<Bytes, TextEditorFsioError>> + Send + 'static,
{
    callback::UploadCallback::<S>::upload(server, client_address, path, content).await
}

pub(super) async fn download_local(
    path: FilePath<PathBuf>,
) -> Result<DownloadStream, TextEditorFsioError> {
    let path = path.full_path();
    validate_download_path(&path)?;
    let file = tokio::fs::File::open(path).await?;
    Ok(Box::pin(stream::unfold(file, |mut file| async {
        let mut buffer = vec![0; DOWNLOAD_CHUNK_SIZE];
        match file.read(&mut buffer).await {
            Ok(0) => None,
            Ok(len) => {
                buffer.truncate(len);
                Some((Ok(Bytes::from(buffer)), file))
            }
            Err(error) => Some((Err(error.into()), file)),
        }
    })))
}

pub(super) async fn upload_local<S>(
    path: FilePath<PathBuf>,
    content: S,
) -> Result<(), TextEditorFsioError>
where
    S: Stream<Item = Result<Bytes, TextEditorFsioError>> + Send + 'static,
{
    let path = path.full_path();
    validate_upload_path(&path)?;
    let mut file = tokio::fs::File::create(path).await?;
    futures::pin_mut!(content);
    while let Some(chunk) = content.next().await {
        file.write_all(&chunk?).await?;
    }
    Ok(())
}

fn validate_download_path(path: &Path) -> Result<(), TextEditorFsioError> {
    if !path.exists() {
        return Err(TextEditorFsioError::PathNotFound {
            path: path.to_owned(),
        });
    }
    if !path.is_file() {
        return Err(TextEditorFsioError::PathNotFile {
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn validate_upload_path(path: &Path) -> Result<(), TextEditorFsioError> {
    if path.is_dir() {
        return Err(TextEditorFsioError::PathIsDirectory {
            path: path.to_owned(),
        });
    }
    if let Some(parent) = path.parent()
        && !parent.is_dir()
    {
        return Err(TextEditorFsioError::ParentDirectoryNotFound {
            path: parent.to_owned(),
        });
    }
    Ok(())
}

impl From<FilePathProto> for FilePath<PathBuf> {
    fn from(proto: FilePathProto) -> Self {
        Self {
            base: PathBuf::from(proto.base),
            file: PathBuf::from(proto.file),
        }
    }
}

impl From<FilePath<PathBuf>> for FilePathProto {
    fn from(path: FilePath<PathBuf>) -> Self {
        Self {
            base: path.base.to_string_lossy().into_owned(),
            file: path.file.to_string_lossy().into_owned(),
        }
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum TextEditorFsioError {
    #[error("[{n}] {0}", n = self.name())]
    IO(#[from] std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    Grpc(#[from] tonic::Status),

    #[error("[{n}] Path not found: {path}", n = self.name(), path = path.display())]
    PathNotFound { path: PathBuf },

    #[error("[{n}] Path is not a file: {path}", n = self.name(), path = path.display())]
    PathNotFile { path: PathBuf },

    #[error("[{n}] Path is a directory: {path}", n = self.name(), path = path.display())]
    PathIsDirectory { path: PathBuf },

    #[error("[{n}] Parent directory not found: {path}", n = self.name(), path = path.display())]
    ParentDirectoryNotFound { path: PathBuf },

    #[error("[{n}] Missing path message", n = self.name())]
    MissingPath,

    #[error("[{n}] Missing address message", n = self.name())]
    MissingAddress,

    #[error("[{n}] Unexpected upload message", n = self.name())]
    UnexpectedUploadMessage,

    #[error("[{n}] Client not found: {0}", n = self.name())]
    RemoteClientNotFound(ClientName),

    #[error("[{n}] Server was not set", n = self.name())]
    ServerNotSet,
}

impl IsGrpcError for TextEditorFsioError {
    fn code(&self) -> Code {
        match self {
            Self::IO { .. } => Code::Internal,
            Self::Grpc(error) => return error.code(),
            Self::PathNotFound { .. } => Code::NotFound,
            Self::PathNotFile { .. }
            | Self::PathIsDirectory { .. }
            | Self::ParentDirectoryNotFound { .. } => Code::FailedPrecondition,
            Self::MissingPath | Self::MissingAddress | Self::UnexpectedUploadMessage => {
                Code::InvalidArgument
            }
            Self::RemoteClientNotFound { .. } => Code::NotFound,
            Self::ServerNotSet => Code::Internal,
        }
    }
}

impl From<TextEditorFsioError> for tonic::Status {
    fn from(error: TextEditorFsioError) -> Self {
        match error {
            TextEditorFsioError::Grpc(status) => status,
            error => tonic::Status::new(error.code(), error.to_string()),
        }
    }
}

impl IsHttpError for TextEditorFsioError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::IO { .. } | Self::Grpc { .. } | Self::ServerNotSet => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Self::PathNotFound { .. }
            | Self::ParentDirectoryNotFound { .. }
            | Self::RemoteClientNotFound { .. } => StatusCode::NOT_FOUND,
            Self::PathNotFile { .. }
            | Self::PathIsDirectory { .. }
            | Self::MissingPath
            | Self::MissingAddress
            | Self::UnexpectedUploadMessage => StatusCode::BAD_REQUEST,
        }
    }
}

pub fn warn_stream_error(error: &TextEditorFsioError) {
    warn!("Text editor fsio stream error: {error}");
}
