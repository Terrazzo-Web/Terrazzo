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
use tokio::io::AsyncWriteExt as _;
use tonic::Code;
use tracing::debug;
use tracing::warn;

use super::File;
use super::FileMetadata;
use super::git;
use crate::backend::client_service::grpc_error::IsGrpcError;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::side::SideViewList;
use crate::text_editor::side::SideViewNode;
use crate::text_editor::side::SvnItem;
use crate::utils::more_path::MorePathRef as _;

const MAX_FILES_SORTED: usize = 5000;
const MAX_FILES_RETURNED: usize = 1000;

pub async fn load_file(path: FilePath<Arc<Path>>) -> Result<Option<File>, FsioError> {
    let path = path.full_path();
    if let Ok(metadata) = path.metadata() {
        if metadata.is_file() {
            if path.extension() == Some("pdf".as_ref()) {
                debug!("Loading PDF file {path:?}");
                let data = tokio::fs::read(&path).await?;
                let base64 = BASE64_STANDARD.encode(data).into();
                return Ok(Some(File::PdfFile {
                    metadata: FileMetadata::single(&path, &metadata).into(),
                    base64,
                }));
            }
            debug!("Loading text file {path:?}");
            let content: Arc<str> = tokio::fs::read_to_string(&path).await?.into();
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

pub async fn list_folder(
    path: FilePath<Arc<Path>>,
) -> Result<Option<Arc<Vec<FileMetadata>>>, FsioError> {
    match load_file(path).await? {
        Some(File::Folder(list)) => Ok(Some(list)),
        _ => Ok(None),
    }
}

pub async fn file_exists(path: FilePath<Arc<Path>>) -> Result<bool, FsioError> {
    Ok(path.full_path().exists())
}

pub async fn prune_side_view(
    base: Arc<Path>,
    tree: Arc<SideViewList<()>>,
) -> Result<Option<Arc<SideViewList<()>>>, FsioError> {
    let (tree, changed) = prune_side_view_rec(&base, PathBuf::new(), tree);
    Ok(changed.then_some(tree))
}

fn prune_side_view_rec(
    base: &Arc<Path>,
    parent_path: PathBuf,
    tree: Arc<SideViewList<()>>,
) -> (Arc<SideViewList<()>>, bool) {
    let mut changed = false;
    let mut new_tree = SideViewList::default();
    for (name, child) in tree.iter() {
        let path = parent_path.join(Path::new(name.as_ref()).make_relative());
        if !(FilePath { base, file: &path }.full_path().exists()) {
            changed = true;
            continue;
        }
        let item = match &child.item {
            SvnItem::Folder { folder, notify: () } => {
                let (folder, folder_changed) = prune_side_view_rec(base, path, folder.clone());
                changed |= folder_changed;
                SvnItem::Folder { folder, notify: () }
            }
            item @ SvnItem::File { metadata: _ } => item.clone(),
        };
        let child = Arc::new(SideViewNode {
            properties: child.properties.clone(),
            item,
        });
        new_tree.insert(name.clone(), child);
    }
    (Arc::new(new_tree), changed)
}

pub async fn store_file(path: FilePath<Arc<Path>>, content: String) -> Result<(), FsioError> {
    let path = path.full_path();
    return if path.exists() {
        Ok(tokio::fs::write(&path, content).await?)
    } else {
        Err(FsioError::PathNotFound { path })
    };
}

pub async fn create_file(path: FilePath<Arc<Path>>, name: String) -> Result<(), FsioError> {
    let name = name.trim();
    let path = create_entry_path(path, &name)?;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await?;
    file.write_all(&format!("-- {name} --").into_bytes())
        .await?;
    Ok(())
}

pub async fn create_folder(path: FilePath<Arc<Path>>, name: String) -> Result<(), FsioError> {
    let name = name.trim();
    let path = create_entry_path(path, &name)?;
    tokio::fs::create_dir(path).await?;
    Ok(())
}

fn create_entry_path(path: FilePath<Arc<Path>>, name: &str) -> Result<PathBuf, FsioError> {
    if name.is_empty() || Path::new(name).components().count() != 1 {
        return Err(FsioError::InvalidEntryName {
            name: name.to_owned(),
        });
    }

    let folder = path.full_path();
    if folder.is_dir() {
        Ok(folder.join(name))
    } else {
        Err(FsioError::ParentNotFolder { path: folder })
    }
}

pub async fn delete_file(
    path: FilePath<Arc<Path>>,
    trash: impl AsRef<Path>,
    git_trash: Option<impl AsRef<Path>>,
) -> Result<(), FsioError> {
    let source = path.full_path();
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
    tokio::fs::create_dir_all(&trash).await?;
    let destination = trash.join(file_name);
    if destination.exists() {
        let metadata = destination.metadata()?;
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let renamed_destination = available_trash_path(&trash, file_name, modified)?;
        tokio::fs::rename(&destination, renamed_destination).await?;

        let new_destination = available_trash_path(&trash, file_name, SystemTime::now())?;
        tokio::fs::rename(source, new_destination).await?;
    } else {
        tokio::fs::rename(source, destination).await?;
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

    #[tokio::test]
    async fn create_file_in_folder() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = FilePath {
            base: Arc::from(tempdir.path()),
            file: Arc::from("".as_ref()),
        };

        super::create_file(path, "  hello world.txt  ".to_owned())
            .await
            .unwrap();

        assert!(tempdir.path().join("hello world.txt").is_file());
    }

    #[tokio::test]
    async fn create_folder_in_folder() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = FilePath {
            base: Arc::from(tempdir.path()),
            file: Arc::from("".as_ref()),
        };

        super::create_folder(path, "new folder".to_owned())
            .await
            .unwrap();

        assert!(tempdir.path().join("new folder").is_dir());
    }

    #[tokio::test]
    async fn create_entry_rejects_nested_names() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = FilePath {
            base: Arc::from(tempdir.path()),
            file: Arc::from("".as_ref()),
        };

        let error = super::create_file(path, "a/b.txt".to_owned())
            .await
            .unwrap_err();

        assert!(matches!(error, super::FsioError::InvalidEntryName { .. }));
    }

    #[tokio::test]
    async fn create_entry_rejects_file_parent() {
        let tempdir = tempfile::tempdir().unwrap();
        tokio::fs::write(tempdir.path().join("parent.txt"), "")
            .await
            .unwrap();
        let path = FilePath {
            base: Arc::from(tempdir.path()),
            file: Arc::from("parent.txt".as_ref()),
        };

        let error = super::create_file(path, "child.txt".to_owned())
            .await
            .unwrap_err();

        assert!(matches!(error, super::FsioError::ParentNotFolder { .. }));
    }

    #[tokio::test]
    async fn store_file_rejects_missing_path() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = FilePath {
            base: Arc::from(tempdir.path()),
            file: Arc::from("missing.txt".as_ref()),
        };

        let error = super::store_file(path, "content".to_owned())
            .await
            .unwrap_err();

        assert!(matches!(error, super::FsioError::PathNotFound { .. }));
    }

    #[tokio::test]
    async fn trash_conflicts_date_existing_and_new_entries() {
        let tempdir = tempfile::tempdir().unwrap();
        let source = tempdir.path().join("source");
        let trash = tempdir.path().join("trash");
        tokio::fs::create_dir(&source).await.unwrap();
        tokio::fs::create_dir(&trash).await.unwrap();
        tokio::fs::write(source.join("file.tar.gz"), "new")
            .await
            .unwrap();
        tokio::fs::write(trash.join("file.tar.gz"), "old")
            .await
            .unwrap();

        let today = chrono::Utc::now().date_naive();
        tokio::fs::write(trash.join(format!("file_{today}.tar.gz")), "first")
            .await
            .unwrap();

        let path = FilePath {
            base: Arc::from(source.as_ref()),
            file: Arc::from("file.tar.gz".as_ref()),
        };

        super::delete_file(path, trash.clone(), None::<&Path>)
            .await
            .unwrap();

        assert!(!source.join("file.tar.gz").exists());
        assert_eq!(
            tokio::fs::read_to_string(trash.join(format!("file_{today}-1.tar.gz")))
                .await
                .unwrap(),
            "old"
        );
        assert_eq!(
            tokio::fs::read_to_string(trash.join(format!("file_{today}-2.tar.gz")))
                .await
                .unwrap(),
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

    #[tokio::test]
    async fn git_file_deletes_to_repo_relative_trash() {
        let tempdir = tempfile::tempdir().unwrap();
        let repo = tempdir.path().join("repo");
        let fallback_trash = tempdir.path().join("fallback-trash");
        tokio::fs::create_dir(&repo).await.unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&repo)
            .status()
            .unwrap();
        tokio::fs::write(repo.join("file.txt"), "deleted")
            .await
            .unwrap();

        let path = FilePath {
            base: Arc::from(repo.as_ref()),
            file: Arc::from("file.txt".as_ref()),
        };

        super::delete_file(path, &fallback_trash, Some(Path::new(".trash")))
            .await
            .unwrap();

        assert!(!repo.join("file.txt").exists());
        assert!(!fallback_trash.exists());
        assert_eq!(
            tokio::fs::read_to_string(repo.join(".trash/file.txt"))
                .await
                .unwrap(),
            "deleted"
        );
    }
}
