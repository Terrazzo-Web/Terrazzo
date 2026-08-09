use std::collections::HashMap;
use std::future::ready;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use futures::FutureExt as _;
use futures::Stream;
use futures::StreamExt as _;
use futures::TryStreamExt as _;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use regex::Regex;
use server_fn::ServerFnError;
use server_fn::codec::TextStream;
use terrazzo::autoclone;
use tracing::Span;
use tracing::debug;
use tracing::info_span;
use tracing::warn;
use tracing_futures::Instrument as _;

use self::tantivy::IndexSettings;
use self::tantivy::SearchIndexError;
use crate::api::client_address::ClientAddress;
use crate::backend::client_service::grpc_error::GrpcError;
use crate::backend::client_service::grpc_error::IsGrpcError;
use crate::backend::client_service::remote_fn_service;
use crate::text_editor::fsio::FileMetadata;
use crate::text_editor::fsio::git::git_repo_root;
use crate::utils::ndjson_utils::serialize_line;

static MAX_RESULTS: usize = 1000;

mod filenames;
mod tantivy;
mod utils;

pub use self::tantivy::reconcile_touched_path;

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
    |server, (base, input)| {
        let settings = server.config().server.with(|server| IndexSettings {
            cache_dir: server.tantivy_cache.clone(),
            refresh_interval: server.search_index_refresh,
            stale_after: server.search_index_stale_after,
        });
        futures::stream::once(async move {
            let span = info_span!("Search", ?base, ?input);
            match search_impl(base, input, settings).instrument(span).await {
                Ok(stream) => stream.left_stream(),
                Err(error) => futures::stream::once(ready(Err(error))).right_stream(),
            }
        })
        .flatten()
        .map_err(GrpcError::from)
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
    SearchIndex(Arc<SearchIndexError>),
}

impl IsGrpcError for SearchError {
    fn code(&self) -> tonic::Code {
        match self {
            Self::Regex { .. } => tonic::Code::InvalidArgument,
            Self::NotGit { .. } => tonic::Code::InvalidArgument,
            Self::InvalidRepoRootPrefix { .. } => tonic::Code::InvalidArgument,
            Self::GitLsFilesError { .. } => tonic::Code::FailedPrecondition,
            Self::SearchIndex { .. } => tonic::Code::Internal,
        }
    }
}

async fn search_impl(
    base: Arc<Path>,
    input: String,
    settings: IndexSettings,
) -> Result<impl Stream<Item = Result<FileMetadata, SearchError>>, SearchError> {
    let repo_root = git_repo_root(base.clone()).ok_or_else(|| SearchError::NotGit(base.clone()))?;

    let filename_search = futures::stream::once(filename_search(
        base.clone(),
        input.clone(),
        repo_root.clone(),
    ))
    .map(|filename_search| match filename_search {
        Ok(filename_search) => filename_search.boxed(),
        Err(error) => {
            warn!("Failed to run Filename search: {error}");
            futures::stream::empty().boxed()
        }
    })
    .flatten()
    .boxed();

    let tantivy_search = futures::stream::once(tantivy_search(base, input, settings, repo_root))
        .map(|tantivy_search| match tantivy_search {
            Ok(tantivy_search) => tantivy_search.boxed(),
            Err(error) => {
                warn!("Failed to run Tantivy search: {error}");
                futures::stream::empty().boxed()
            }
        })
        .flatten()
        .boxed();

    Ok(futures::stream::iter([filename_search, tantivy_search])
        .flatten_unordered(None)
        .take(MAX_RESULTS))
}

#[autoclone]
async fn filename_search(
    base: Arc<Path>,
    input: String,
    repo_root: Arc<Path>,
) -> Result<impl Stream<Item = Result<FileMetadata, SearchError>> + Send, SearchError> {
    let regex =
        Arc::new(Regex::new(&input).map_err(|error| SearchError::Regex(input.clone(), error))?);
    Ok(self::utils::git_files(repo_root.clone(), base.clone())
        .await?
        .filter_map(move |path| {
            autoclone!(base);
            filenames::process_path(base.clone(), path, regex.clone())
                .map(|maybe| maybe.transpose())
        }))
}

async fn tantivy_search(
    base: Arc<Path>,
    input: String,
    settings: IndexSettings,
    repo_root: Arc<Path>,
) -> Result<impl Stream<Item = Result<FileMetadata, SearchError>> + Send, SearchError> {
    let index = tantivy::repository_index(repo_root.clone(), settings)
        .await
        .map_err(SearchError::SearchIndex)?;
    let content_paths = index
        .search(&input, MAX_RESULTS)
        .map_err(|error| SearchError::SearchIndex(Arc::new(error)))?;
    index.refresh_if_stale();
    let content_matches =
        futures::stream::iter(content_paths.into_iter().map(Ok)).filter_map(move |path| {
            process_index_path(repo_root.clone(), base.clone(), path).map(|maybe| maybe.transpose())
        });
    Ok(content_matches)
}

async fn process_index_path(
    repo_root: Arc<Path>,
    base: Arc<Path>,
    path: Result<PathBuf, SearchError>,
) -> Result<Option<FileMetadata>, SearchError> {
    let path = path?;
    let base_from_root = base
        .strip_prefix(&repo_root)
        .map_err(SearchError::InvalidRepoRootPrefix)?;
    let Ok(path_from_base) = path.strip_prefix(base_from_root) else {
        return Ok(None);
    };
    let full_path = repo_root.join(&path);
    let Some(metadata) = tokio::fs::symlink_metadata(&full_path).await.ok() else {
        reconcile_touched_path(&full_path);
        return Ok(None);
    };
    let result = FileMetadata::make(
        path_from_base.display().to_string().into(),
        Ok(&metadata),
        &mut HashMap::new(),
        &mut HashMap::new(),
    );
    Ok(Some(result))
}
