use std::ffi::OsString;
use std::fs;
use std::future::ready;
use std::os::unix::ffi::OsStringExt as _;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use futures::Stream;
use futures::StreamExt;
use futures::TryStreamExt as _;
use regex::Regex;
use server_fn::ServerFnError;
use server_fn::codec::TextStream;
use tonic::Status;
use tracing::debug;
use tracing::warn;

use crate::api::client_address::ClientAddress;
use crate::backend::client_service::remote_fn_service;
use crate::text_editor::fsio::FileMetadata;
use crate::text_editor::fsio::git::git_repo_root;
use crate::utils::ndjson_utils::serialize_line;

pub async fn search(
    remote: ClientAddress,
    base: Arc<Path>,
    input: String,
) -> Result<TextStream, ServerFnError> {
    debug!(%remote, "Calling search({base:?}, {input:?})");
    let stream = SEARCH_FN.call(remote, (base, input)).await?;
    let stream = stream.filter_map(|item| {
        let item = item.map(|item| {
            serialize_line(&item)
                .inspect_err(|error| warn!("Failed to serialize: {error}"))
                .ok()
        });
        let item = item.transpose();
        return ready(item);
    });
    Ok(TextStream::new(stream.map_err(Into::into)))
}

remote_fn_service::streaming::declare_remote_fn!(
    SEARCH_FN,
    "texteditor.search",
    (Arc<Path>, String),
    FileMetadata,
    |_server, (base, input)| search_impl(base, input)
);

fn search_impl(base: Arc<Path>, input: String) -> impl Stream<Item = Result<FileMetadata, Status>> {
    let regex = match Regex::new(&input) {
        Ok(regex) => regex,
        Err(error) => {
            return futures::stream::iter(vec![Ok(FileMetadata {
                name: error.to_string().into(),
                ..FileMetadata::default()
            })]);
        }
    };

    let paths = match git_repo_root(&base) {
        Some(repo_root) => git_files(&repo_root, &base).unwrap_or_default(),
        None => files_recursively(&base),
    };
    let results: Vec<Result<FileMetadata, Status>> = paths
        .into_iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(base.as_ref()).ok()?;
            let name = relative.to_string_lossy();
            regex.is_match(&name).then(|| {
                let metadata = fs::symlink_metadata(&path).ok()?;
                let mut result = FileMetadata::single(&path, &metadata);
                result.name = name.into_owned().into();
                Some(Ok::<_, Status>(result))
            })?
        })
        .collect();
    futures::stream::iter(results)
}

fn git_files(repo_root: &Path, base: &Path) -> Option<Vec<PathBuf>> {
    let relative_base = base.strip_prefix(repo_root).ok()?;
    let pathspec = if relative_base.as_os_str().is_empty() {
        Path::new(".")
    } else {
        relative_base
    };
    let output = Command::new("git")
        .current_dir(repo_root)
        .arg("--literal-pathspecs")
        .args(["ls-files", "-z"])
        .args(["--cached", "--others", "--exclude-standard", "--"])
        .arg(pathspec)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    Some(
        output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| repo_root.join(OsString::from_vec(path.to_owned())))
            .collect(),
    )
}

fn files_recursively(base: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut directories = vec![base.to_owned()];
    while let Some(directory) = directories.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(file_type) if file_type.is_dir() => directories.push(entry.path()),
                Ok(file_type) if file_type.is_file() || file_type.is_symlink() => {
                    files.push(entry.path());
                }
                Ok(_) | Err(_) => {}
            }
        }
    }
    files
}
