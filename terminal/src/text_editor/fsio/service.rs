#![cfg(feature = "server")]

use std::cmp::Reverse;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
use chrono::DateTime;
use chrono::Utc;
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
            let original = git::git_repo_root(&path)
                .is_some()
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
        Err(FsioError::PathNotFound { path })
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

pub fn delete_file(
    path: FilePath<Arc<str>>,
    trash: impl AsRef<Path>,
    git_trash: Option<impl AsRef<Path>>,
) -> Result<(), FsioError> {
    let source = concat_base_file_path(path.base, path.file);
    if !source.exists() {
        return Err(FsioError::PathNotFound { path: source });
    }
    let Some(file_name) = source.file_name() else {
        return Err(FsioError::MissingFileName { path: source });
    };

    let trash = delete_trash_path(
        &source,
        trash.as_ref(),
        git_trash.as_ref().map(AsRef::as_ref),
    );
    std::fs::create_dir_all(&trash)?;
    let destination = trash.join(file_name);
    if destination.exists() {
        let metadata = destination.metadata()?;
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let renamed_destination = available_trash_path(&trash, file_name, modified)?;
        std::fs::rename(&destination, renamed_destination)?;

        let new_destination = available_trash_path(&trash, file_name, SystemTime::now())?;
        std::fs::rename(source, new_destination)?;
    } else {
        std::fs::rename(source, destination)?;
    }
    Ok(())
}

fn delete_trash_path(source: &Path, trash: &Path, git_trash: Option<&Path>) -> PathBuf {
    if let Some(git_trash) = git_trash
        && let Some(repo_root) = git::git_repo_root(source)
    {
        return repo_root.join(git_trash);
    }
    trash.to_owned()
}

fn create_entry_path(path: FilePath<Arc<str>>, name: &str) -> Result<PathBuf, FsioError> {
    let name = name.trim();
    if name.is_empty() || Path::new(name).components().count() != 1 {
        return Err(FsioError::InvalidEntryName {
            name: name.to_owned(),
        });
    }

    let folder = concat_base_file_path(path.base, path.file);
    if folder.is_dir() {
        Ok(folder.join(name))
    } else {
        Err(FsioError::ParentNotFolder { path: folder })
    }
}

fn available_trash_path(
    trash: &Path,
    file_name: &std::ffi::OsStr,
    time: SystemTime,
) -> Result<PathBuf, FsioError> {
    let file_name = file_name
        .to_str()
        .ok_or_else(|| FsioError::NonUnicodeFileName {
            file_name: file_name.into(),
        })?;
    let date = DateTime::<Utc>::from(time).date_naive();
    let (name, extension) = split_archive_extension(file_name);
    for suffix in std::iter::once(String::new()).chain((1..).map(|index| format!("-{index}"))) {
        let candidate = if extension.is_empty() {
            format!("{name}_{date}{suffix}")
        } else {
            format!("{name}_{date}{suffix}.{extension}")
        };
        let candidate = trash.join(candidate);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    unreachable!()
}

fn split_archive_extension(file_name: &str) -> (&str, &str) {
    match file_name
        .char_indices()
        .find(|(index, c)| *index > 0 && *c == '.')
    {
        Some((index, _)) => (&file_name[..index], &file_name[index + 1..]),
        None => (file_name, ""),
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum FsioError {
    #[error("[{n}] {0}", n = self.name())]
    IO(#[from] std::io::Error),

    #[error("[{n}] Path not found: {path:?}", n = self.name())]
    PathNotFound { path: PathBuf },

    #[error("[{n}] Path has no file name: {path:?}", n = self.name())]
    MissingFileName { path: PathBuf },

    #[error("[{n}] Invalid entry name: {name:?}", n = self.name())]
    InvalidEntryName { name: String },

    #[error("[{n}] Parent is not a folder: {path:?}", n = self.name())]
    ParentNotFolder { path: PathBuf },

    #[error("[{n}] File name is not valid Unicode: {file_name:?}", n = self.name())]
    NonUnicodeFileName { file_name: OsString },
}

impl IsGrpcError for FsioError {
    fn code(&self) -> Code {
        match self {
            Self::IO { .. } => Code::FailedPrecondition,
            Self::PathNotFound { .. } => Code::NotFound,
            Self::MissingFileName { .. } => Code::InvalidArgument,
            Self::InvalidEntryName { .. } => Code::InvalidArgument,
            Self::ParentNotFolder { .. } => Code::FailedPrecondition,
            Self::NonUnicodeFileName { .. } => Code::InvalidArgument,
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
    use std::path::Path;
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

        assert!(matches!(error, super::FsioError::InvalidEntryName { .. }));
    }

    #[test]
    fn create_entry_rejects_file_parent() {
        let tempdir = tempfile::tempdir().unwrap();
        std::fs::write(tempdir.path().join("parent.txt"), "").unwrap();
        let path = FilePath {
            base: Arc::from(tempdir.path().to_string_lossy().to_string()),
            file: Arc::from("parent.txt"),
        };

        let error = super::create_file(path, "child.txt".to_owned()).unwrap_err();

        assert!(matches!(error, super::FsioError::ParentNotFolder { .. }));
    }

    #[test]
    fn store_file_rejects_missing_path() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = FilePath {
            base: Arc::from(tempdir.path().to_string_lossy().to_string()),
            file: Arc::from("missing.txt"),
        };

        let error = super::store_file(path, "content".to_owned()).unwrap_err();

        assert!(matches!(error, super::FsioError::PathNotFound { .. }));
    }

    #[test]
    fn trash_conflicts_date_existing_and_new_entries() {
        let tempdir = tempfile::tempdir().unwrap();
        let source = tempdir.path().join("source");
        let trash = tempdir.path().join("trash");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&trash).unwrap();
        std::fs::write(source.join("file.tar.gz"), "new").unwrap();
        std::fs::write(trash.join("file.tar.gz"), "old").unwrap();

        let today = chrono::Utc::now().date_naive();
        std::fs::write(trash.join(format!("file_{today}.tar.gz")), "first").unwrap();

        let path = FilePath {
            base: Arc::from(source.to_string_lossy().to_string()),
            file: Arc::from("file.tar.gz"),
        };

        super::delete_file(path, trash.clone(), None::<&Path>).unwrap();

        assert!(!source.join("file.tar.gz").exists());
        assert_eq!(
            std::fs::read_to_string(trash.join(format!("file_{today}-1.tar.gz"))).unwrap(),
            "old"
        );
        assert_eq!(
            std::fs::read_to_string(trash.join(format!("file_{today}-2.tar.gz"))).unwrap(),
            "new"
        );
    }

    #[test]
    fn split_archive_extension_keeps_tar_gz_together() {
        assert_eq!(
            super::split_archive_extension("file.tar.gz"),
            ("file", "tar.gz")
        );
        assert_eq!(super::split_archive_extension("file"), ("file", ""));
        assert_eq!(super::split_archive_extension(".env"), (".env", ""));
    }

    #[test]
    fn git_file_deletes_to_repo_relative_trash() {
        let tempdir = tempfile::tempdir().unwrap();
        let repo = tempdir.path().join("repo");
        let fallback_trash = tempdir.path().join("fallback-trash");
        std::fs::create_dir(&repo).unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&repo)
            .status()
            .unwrap();
        std::fs::write(repo.join("file.txt"), "deleted").unwrap();

        let path = FilePath {
            base: Arc::from(repo.to_string_lossy().to_string()),
            file: Arc::from("file.txt"),
        };

        super::delete_file(path, &fallback_trash, Some(Path::new(".trash"))).unwrap();

        assert!(!repo.join("file.txt").exists());
        assert!(!fallback_trash.exists());
        assert_eq!(
            std::fs::read_to_string(repo.join(".trash/file.txt")).unwrap(),
            "deleted"
        );
    }
}
