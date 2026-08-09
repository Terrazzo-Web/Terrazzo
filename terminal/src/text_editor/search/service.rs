use std::ffi::OsString;
use std::fs;
use std::future::ready;
use std::io::ErrorKind;
use std::os::unix::ffi::OsStringExt as _;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use futures::Stream;
use futures::StreamExt as _;
use futures::TryStreamExt as _;
use nameth::NamedEnumValues as _;
use nameth::nameth;
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
    |_server, (base, input)| match search_impl(base, input) {
        Ok(stream) => stream.left_stream(),
        Err(error) => futures::stream::once(ready(Err(Status::internal(format!(
            "Search worker failed: {error}"
        )))))
        .right_stream(),
    }
);

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum SearchError {
    #[error("[{n}] Failed to parse regular expression '{0}': {1}", n = self.name())]
    Regex(String, regex::Error),

    #[error("[{n}] {0}", n = self.name())]
    NotGit(Arc<Path>),

    #[error("[{n}] {0}", n = self.name())]
    InvalidRepoRootPrefix(std::path::StripPrefixError),

    #[error("[{n}] {0}", n = self.name())]
    GitLsFilesError(std::io::Error),

    #[error("[{n}] {0}", n = self.name())]
    GitLsFilesExit(std::process::ExitStatus),
}

fn search_impl(
    base: Arc<Path>,
    input: String,
) -> Result<impl Stream<Item = Result<FileMetadata, SearchError>>, SearchError> {
    let regex = Regex::new(&input).map_err(|error| SearchError::Regex(input, error))?;

    let repo_root = git_repo_root(&base).ok_or_else(|| SearchError::NotGit(base))?;
    let paths = git_files(&repo_root, &base);
    paths
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
        .collect()
}

async fn git_files(repo_root: &Path, base: &Path) -> Result<Vec<PathBuf>, SearchError> {
    let pathspec = base
        .strip_prefix(repo_root)
        .map_err(SearchError::InvalidRepoRootPrefix)?;
    let pathspec = if pathspec.as_os_str().is_empty() {
        Path::new(".")
    } else {
        pathspec
    };
    let process = tokio::process::Command::new("git")
        .current_dir(repo_root)
        .arg("--literal-pathspecs")
        .args(["ls-files", "-z"])
        .args(["--cached", "--others", "--exclude-standard", "--"])
        .arg(pathspec)
        .spawn()
        .map_err(SearchError::GitLsFilesError)?;
    let output = process.stdout.ok_or_else(|| {
        SearchError::GitLsFilesError(std::io::Error::new(
            ErrorKind::BrokenPipe,
            "git ls-files didn't return anything",
        ))
    })?;
    tokio_stream::
    if !output.status.success() {
        return Err(SearchError::GitLsFilesExit(output.status));
    }

    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| repo_root.join(OsString::from_vec(path.to_owned())))
        .collect())
}
