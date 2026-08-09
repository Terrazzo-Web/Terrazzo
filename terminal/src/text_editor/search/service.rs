use std::future::ready;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use futures::FutureExt;
use futures::Stream;
use futures::StreamExt as _;
use futures::TryStreamExt as _;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use regex::Regex;
use server_fn::ServerFnError;
use server_fn::codec::TextStream;
use tokio::io::AsyncBufReadExt as _;
use tokio::io::BufReader;
use tokio_stream::wrappers::LinesStream;
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
    |_server, (base, input)| futures::stream::once(async move {
        match search_impl(base, input).await {
            Ok(stream) => stream.left_stream(),
            Err(error) => futures::stream::once(ready(Err(error))).right_stream(),
        }
    })
    .flatten()
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

async fn search_impl(
    base: Arc<Path>,
    input: String,
) -> Result<impl Stream<Item = Result<FileMetadata, SearchError>>, SearchError> {
    let regex = Arc::new(Regex::new(&input).map_err(|error| SearchError::Regex(input, error))?);
    let repo_root = git_repo_root(base.clone()).ok_or_else(|| SearchError::NotGit(base.clone()))?;
    let paths = git_files(repo_root.clone(), base.clone()).await?;
    Ok(paths.filter_map(move |path| {
        process_path(base.clone(), path, regex.clone()).map(|maybe| maybe.transpose())
    }))
}

async fn process_path(
    base: Arc<Path>,
    path: Result<PathBuf, SearchError>,
    regex: Arc<Regex>,
) -> Result<Option<FileMetadata>, SearchError> {
    let base = base.clone();
    let path = path?;
    let relative = path
        .strip_prefix(&base)
        .map_err(SearchError::InvalidRepoRootPrefix)?;
    let name = relative.to_string_lossy();
    if !regex.is_match(&name) {
        return Ok(None);
    }
    let Some(metadata) = tokio::fs::symlink_metadata(&path).await.ok() else {
        return Ok(None);
    };
    let result = FileMetadata::single(&path, &metadata);
    Ok(Some(result))
}

async fn git_files(
    repo_root: Arc<Path>,
    base: Arc<Path>,
) -> Result<impl Stream<Item = Result<PathBuf, SearchError>>, SearchError> {
    let pathspec = base
        .strip_prefix(&repo_root)
        .map_err(SearchError::InvalidRepoRootPrefix)?;
    let pathspec = if pathspec.as_os_str().is_empty() {
        Path::new(".")
    } else {
        pathspec
    };
    let process = tokio::process::Command::new("git")
        .current_dir(&repo_root)
        .arg("--literal-pathspecs")
        .args(["ls-files", "-z"])
        .args(["--cached", "--others", "--exclude-standard", "--"])
        .arg(pathspec)
        .spawn()
        .map_err(SearchError::GitLsFilesError)?;
    let stdout = process.stdout.ok_or_else(|| {
        SearchError::GitLsFilesError(std::io::Error::new(
            ErrorKind::BrokenPipe,
            "git ls-files didn't return anything",
        ))
    })?;

    let lines = BufReader::new(stdout).lines();
    let stream = LinesStream::new(lines);
    let stream = stream.map(|line| line.map_err(SearchError::GitLsFilesError));
    let stream = stream.map_ok(PathBuf::from);
    Ok(stream)
}
