use std::path::Path;
use std::sync::Arc;

use server_fn::ServerFnError;
use tantivy::query::QueryParser;
use tantivy::snippet::SnippetGenerator;
use tracing::debug;

use super::IndexSettings;
use super::SearchError;
use crate::api::client_address::ClientAddress;
use crate::backend::client_service::grpc_error::GrpcError;
use crate::backend::client_service::remote_fn_service;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio::CursorPosition;
use crate::text_editor::fsio::git::git_repo_root;

pub async fn get_highlight_ranges(
    remote: ClientAddress,
    path: FilePath<Arc<Path>>,
    input: String,
) -> Result<Vec<CursorPosition>, ServerFnError> {
    Ok(GET_HIGHLIGHT_RANGES_FN.call(remote, (path, input)).await?)
}

remote_fn_service::unary::declare_remote_fn!(
    GET_HIGHLIGHT_RANGES_FN,
    "texteditor.search.get_highlight_ranges",
    (FilePath<Arc<Path>>, String),
    Vec<CursorPosition>,
    |server, (path, input)| {
        let settings = server.config().server.with(|server| IndexSettings {
            cache_dir: server.tantivy_cache.clone(),
            refresh_interval: server.search_index_refresh,
            stale_after: server.search_index_stale_after,
        });
        async move {
            get_highlight_ranges_impl(path, input, settings)
                .await
                .map_err(GrpcError::from)
        }
    }
);

async fn get_highlight_ranges_impl(
    path: FilePath<Arc<Path>>,
    input: String,
    settings: IndexSettings,
) -> Result<Vec<CursorPosition>, SearchError> {
    let full_path = path.full_path();
    let repo_root =
        git_repo_root(&full_path).ok_or_else(|| SearchError::NotGit(path.base.clone()))?;
    let index = super::tantivy::repository_index(repo_root, settings)
        .await
        .map_err(SearchError::SearchIndex)?;
    let text = tokio::fs::read_to_string(full_path)
        .await
        .map_err(|error| SearchError::SearchIndex(Arc::new(error.into())))?;
    highlight_ranges(&index, &input, &text)
        .map_err(|error| SearchError::SearchIndex(Arc::new(error)))
}

fn highlight_ranges(
    repository: &super::tantivy::RepositoryIndex,
    input: &str,
    text: &str,
) -> Result<Vec<CursorPosition>, super::tantivy::SearchIndexError> {
    highlight_ranges_in_index(
        &repository.index,
        &repository.reader,
        repository.fields.body,
        input,
        text,
    )
}

fn highlight_ranges_in_index(
    index: &tantivy::Index,
    reader: &tantivy::IndexReader,
    body: tantivy::schema::Field,
    input: &str,
    text: &str,
) -> Result<Vec<CursorPosition>, super::tantivy::SearchIndexError> {
    if input.is_empty() || text.is_empty() {
        return Ok(vec![]);
    }

    let searcher = reader.searcher();
    let mut parser = QueryParser::for_index(index, vec![body]);
    parser.set_conjunction_by_default();
    let (query, errors) = parser.parse_query_lenient(input);
    for error in errors {
        debug!(%error, "Ignoring Tantivy query parser error");
    }

    let mut generator = SnippetGenerator::create(&searcher, &*query, body)?;
    // Tantivy's highlighted offsets are relative to the selected snippet. A snippet this
    // large cannot split, so its first (and only) fragment starts at the start of `text`.
    generator.set_max_num_chars(text.len());
    let snippet = generator.snippet(text);
    Ok(snippet
        .highlighted()
        .iter()
        .filter_map(|range| byte_range_to_cursor_position(text, range.clone()))
        .collect())
}

fn byte_range_to_cursor_position(
    text: &str,
    range: std::ops::Range<usize>,
) -> Option<CursorPosition> {
    let prefix = text.get(..range.start)?;
    let matched = text.get(range)?;
    let anchor = u32::try_from(prefix.encode_utf16().count()).ok()?;
    let length = u32::try_from(matched.encode_utf16().count()).ok()?;
    Some(CursorPosition {
        anchor,
        head: anchor.checked_add(length)?,
    })
}

#[cfg(test)]
mod tests {
    use super::byte_range_to_cursor_position;
    use super::highlight_ranges_in_index;
    use crate::text_editor::fsio::CursorPosition;

    #[test]
    fn converts_utf8_byte_ranges_to_codemirror_offsets() {
        let text = "a🦀bcafé";
        let range = text.find("café").unwrap();
        let actual = byte_range_to_cursor_position(text, range..text.len()).unwrap();
        let CursorPosition { anchor, head } = actual;
        assert_eq!((4, 8), (anchor, head));
    }

    #[test]
    fn highlights_all_matches_in_the_document() {
        let mut schema = tantivy::schema::Schema::builder();
        let body = schema.add_text_field("body", tantivy::schema::TEXT);
        let index = tantivy::Index::create_in_ram(schema.build());
        let mut writer = index.writer_with_num_threads(1, 20_000_000).unwrap();
        writer
            .add_document(tantivy::doc!(body => "alpha café alpha"))
            .unwrap();
        writer.commit().unwrap();
        let reader = index.reader().unwrap();

        let actual =
            highlight_ranges_in_index(&index, &reader, body, "alpha", "🦀 alpha café alpha")
                .unwrap();
        let actual = actual
            .into_iter()
            .map(|position| (position.anchor, position.head))
            .collect::<Vec<_>>();

        assert_eq!(vec![(3, 8), (14, 19)], actual);
    }
}
