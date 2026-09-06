use std::path::Path;
use std::sync::Arc;

use server_fn::ServerFnError;

use super::SearchError;
use crate::api::client_address::ClientAddress;
use crate::backend::client_service::grpc_error::GrpcError;
use crate::backend::client_service::remote_fn_service;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio::CursorPosition;

pub async fn get_highlight_ranges(
    remote: ClientAddress,
    path: FilePath<Arc<Path>>,
    input: String,
) -> Result<Option<CursorPosition>, ServerFnError> {
    Ok(GET_HIGHLIGHT_RANGES_FN.call(remote, (path, input)).await?)
}

remote_fn_service::unary::declare_remote_fn!(
    GET_HIGHLIGHT_RANGES_FN,
    "texteditor.search.get_highlight_ranges",
    (FilePath<Arc<Path>>, String),
    Option<CursorPosition>,
    |_server, (path, input)| {
        async move {
            get_highlight_ranges_impl(path, input)
                .await
                .map_err(GrpcError::from)
        }
    }
);

async fn get_highlight_ranges_impl(
    path: FilePath<Arc<Path>>,
    input: String,
) -> Result<Option<CursorPosition>, SearchError> {
    let full_path = path.full_path();
    let text = tokio::fs::read_to_string(full_path)
        .await
        .map_err(|error| SearchError::SearchIndex(Arc::new(error.into())))?;
    Ok(highlight_range(&input, &text))
}

fn highlight_range(input: &str, text: &str) -> Option<CursorPosition> {
    if input.is_empty() || text.is_empty() {
        return None;
    }

    let start = text.find(input)?;
    byte_range_to_cursor_position(text, start..start + input.len())
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
    use super::highlight_range;
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
    fn highlights_the_first_literal_occurrence() {
        let actual = highlight_range(
            "futures = { workspace = true, optional = true }",
            "🦀\nworkspace = true\nfutures = { workspace = true, optional = true }\n",
        )
        .unwrap();

        assert_eq!((20, 67), (actual.anchor, actual.head));
    }

    #[test]
    fn returns_none_when_the_input_is_empty_or_absent() {
        assert!(highlight_range("", "some text").is_none());
        assert!(highlight_range("missing", "some text").is_none());
    }
}
