#![cfg(feature = "server")]

use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use tonic::Code;
use tracing::debug;
use tracing::warn;

use super::File;
use super::FileMetadata;
use super::canonical::concat_base_file_path;
use super::git;
use crate::backend::client_service::grpc_error::IsGrpcError;
use crate::text_editor::file_path::FilePath;

const MAX_FILES_SORTED: usize = 5000;
const MAX_FILES_RETURNED: usize = 1000;

pub fn load_file(path: FilePath<Arc<str>>) -> Result<Option<File>, FsioError> {
    let path = concat_base_file_path(path.base, path.file);
    if let Ok(metadata) = path.metadata() {
        if metadata.is_file() {
            if path.extension() == Some("pdf".as_ref()) {
                debug!("Loading PDF file {path:?}");
                let data = std::fs::read(&path)?;
                let base64 = BASE64_STANDARD.encode(data).into();
                return Ok(Some(File::PdfFile {
                    metadata: FileMetadata::single(&path, &metadata).into(),
                    base64,
                }));
            }
            debug!("Loading text file {path:?}");
            let content: Arc<str> = std::fs::read_to_string(&path)?.into();
            let original = git::is_in_git_repo(&path)
                .then(|| {
                    git::file_content_at_commit(&path, "HEAD")
                        .inspect_err(|error| warn!("Failed to load git file: {error}"))
                        .ok()
                })
                .flatten()
                .filter(|original| original != content.as_ref())
                .map(Arc::from);
            return Ok(Some(File::TextFile {
                metadata: FileMetadata::single(&path, &metadata).into(),
                original,
                content,
            }));
        }
        if metadata.is_dir() {
            debug!("Loading file {path:?}");
            let mut files = vec![];
            let mut uids = HashMap::default();
            let mut gids = HashMap::default();
            for file in path
                .read_dir()?
                .filter_map(|f| f.ok())
                .take(MAX_FILES_SORTED)
            {
                files.push(FileMetadata::of(file, &mut uids, &mut gids));
            }
            files.sort_by_key(|f| Reverse(f.modified));
            let mut files = files
                .into_iter()
                .take(MAX_FILES_RETURNED)
                .collect::<Vec<_>>();
            files.sort_by(|a, b| Ord::cmp(&a.name, &b.name));
            return Ok(Some(File::Folder(Arc::from(files))));
        }
    }
    debug!("Not found {path:?}");
    Ok(None)
}

pub fn list_folder(path: FilePath<Arc<str>>) -> Result<Option<Arc<Vec<FileMetadata>>>, FsioError> {
    match load_file(path)? {
        Some(File::Folder(list)) => Ok(Some(list)),
        _ => Ok(None),
    }
}

pub fn store_file(path: FilePath<Arc<str>>, content: String) -> Result<(), FsioError> {
    let path = concat_base_file_path(path.base, path.file);
    return if path.exists() {
        Ok(std::fs::write(&path, content)?)
    } else {
        Err(FsioError::InvalidPath)
    };
}

pub fn create_file(path: FilePath<Arc<str>>, name: String) -> Result<(), FsioError> {
    let path = create_entry_path(path, &name)?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    Ok(())
}

pub fn create_folder(path: FilePath<Arc<str>>, name: String) -> Result<(), FsioError> {
    let path = create_entry_path(path, &name)?;
    std::fs::create_dir(path)?;
    Ok(())
}

fn create_entry_path(path: FilePath<Arc<str>>, name: &str) -> Result<PathBuf, FsioError> {
    let name = name.trim();
    if name.is_empty() || Path::new(name).components().count() != 1 {
        return Err(FsioError::InvalidPath);
    }

    let folder = concat_base_file_path(path.base, path.file);
    if folder.is_dir() {
        Ok(folder.join(name))
    } else {
        Err(FsioError::InvalidPath)
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum FsioError {
    #[error("[{n}] {0}", n = self.name())]
    IO(#[from] std::io::Error),

    #[error("[{n}] Invalid path", n = self.name())]
    InvalidPath,
}

impl IsGrpcError for FsioError {
    fn code(&self) -> Code {
        match self {
            Self::IO { .. } => Code::FailedPrecondition,
            Self::InvalidPath => Code::InvalidArgument,
        }
    }
}

#[cfg(test)]
#[test]
fn check_option_order() {
    assert!(None < Some(-2));
    assert!(Some(1) < Some(2));
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::text_editor::file_path::FilePath;

    #[test]
    fn create_file_in_folder() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = FilePath {
            base: Arc::from(tempdir.path().to_string_lossy().to_string()),
            file: Arc::from(""),
        };

        super::create_file(path, "  hello world.txt  ".to_owned()).unwrap();

        assert!(tempdir.path().join("hello world.txt").is_file());
    }

    #[test]
    fn create_folder_in_folder() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = FilePath {
            base: Arc::from(tempdir.path().to_string_lossy().to_string()),
            file: Arc::from(""),
        };

        super::create_folder(path, "new folder".to_owned()).unwrap();

        assert!(tempdir.path().join("new folder").is_dir());
    }

    #[test]
    fn create_entry_rejects_nested_names() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = FilePath {
            base: Arc::from(tempdir.path().to_string_lossy().to_string()),
            file: Arc::from(""),
        };

        let error = super::create_file(path, "a/b.txt".to_owned()).unwrap_err();

        assert!(matches!(error, super::FsioError::InvalidPath));
    }
}
