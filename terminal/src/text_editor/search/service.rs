use std::collections::HashMap;
use std::future::ready;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
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
use tracing::Span;
use tracing::debug;
use tracing::info_span;
use tracing::warn;
use tracing_futures::Instrument as _;

use crate::api::client_address::ClientAddress;
use crate::backend::client_service::grpc_error::GrpcError;
use crate::backend::client_service::grpc_error::IsGrpcError;
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
    Ok(TextStream::new(
        stream.map_err(Into::into).instrument(Span::current()),
    ))
}

remote_fn_service::streaming::declare_remote_fn!(
    SEARCH_FN,
    "texteditor.search",
    (Arc<Path>, String),
    FileMetadata,
    |_server, (base, input)| futures::stream::once(async move {
        let span = info_span!("Search", ?base, ?input);
        match search_impl(base, input).instrument(span).await {
            Ok(stream) => stream.left_stream(),
            Err(error) => futures::stream::once(ready(Err(error))).right_stream(),
        }
    })
    .flatten()
    .map_err(GrpcError::from)
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
}

impl IsGrpcError for SearchError {
    fn code(&self) -> tonic::Code {
        match self {
            Self::Regex { .. } => tonic::Code::InvalidArgument,
            Self::NotGit { .. } => tonic::Code::InvalidArgument,
            Self::InvalidRepoRootPrefix { .. } => tonic::Code::InvalidArgument,
            Self::GitLsFilesError { .. } => tonic::Code::FailedPrecondition,
        }
    }
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
    let path = path?;
    let Some(name) = path.file_name() else {
        return Ok(None);
    };
    if !regex.is_match(&name.to_string_lossy()) {
        debug!("Not match: {path:?}");
        return Ok(None);
    }
    let full_path = base.join(&path);
    let Some(metadata) = tokio::fs::symlink_metadata(&full_path).await.ok() else {
        debug!("Failed to load metadata for {full_path:?}");
        return Ok(None);
    };
    debug!("Match: {full_path:?} metadata={metadata:?}");
    let result = FileMetadata::make(
        path.display().to_string().into(),
        Ok(&metadata),
        &mut HashMap::new(),
        &mut HashMap::new(),
    );
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
    }
    .to_owned();
    let process = tokio::process::Command::new("git")
        .current_dir(&repo_root)
        .arg("--literal-pathspecs")
        .args(["ls-files"])
        .args(["--cached", "--others", "--exclude-standard", "--"])
        .arg(&pathspec)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
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
    let stream = stream.map(move |line| {
        if pathspec == Path::new(".") {
            return Ok(PathBuf::from(line?));
        }
        Ok(PathBuf::from(line?)
            .strip_prefix(&pathspec)
            .map_err(SearchError::InvalidRepoRootPrefix)?
            .to_owned())
    });
    #[cfg(debug_assertions)]
    let stream = stream.inspect(|row| debug!("Row: {row:?}"));
    Ok(stream)
}
