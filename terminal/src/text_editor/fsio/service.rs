#![cfg(feature = "server")]

use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

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

pub fn delete_file(path: FilePath<Arc<str>>, trash: PathBuf) -> Result<(), FsioError> {
    let source = concat_base_file_path(path.base, path.file);
    if !source.exists() {
        return Err(FsioError::InvalidPath);
    }
    let Some(file_name) = source.file_name() else {
        return Err(FsioError::InvalidPath);
    };

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

fn available_trash_path(
    trash: &Path,
    file_name: &std::ffi::OsStr,
    time: SystemTime,
) -> Result<PathBuf, FsioError> {
    let file_name = file_name.to_str().ok_or(FsioError::InvalidPath)?;
    let date = system_time_date(time);
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

fn system_time_date(time: SystemTime) -> String {
    let days = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
        / 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year as i32, month as u32, day as u32)
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
    use std::time::SystemTime;

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

    #[test]
    fn trash_conflicts_date_existing_and_new_entries() {
        let tempdir = tempfile::tempdir().unwrap();
        let source = tempdir.path().join("source");
        let trash = tempdir.path().join("trash");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&trash).unwrap();
        std::fs::write(source.join("file.tar.gz"), "new").unwrap();
        std::fs::write(trash.join("file.tar.gz"), "old").unwrap();

        let today = super::system_time_date(SystemTime::now());
        std::fs::write(trash.join(format!("file_{today}.tar.gz")), "first").unwrap();

        let path = FilePath {
            base: Arc::from(source.to_string_lossy().to_string()),
            file: Arc::from("file.tar.gz"),
        };

        super::delete_file(path, trash.clone()).unwrap();

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
}
