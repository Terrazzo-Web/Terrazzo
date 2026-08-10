use std::path::Path;
use std::sync::Arc;

use server_fn::Http;
use server_fn::ServerFnError;
use server_fn::codec::Json;
use server_fn::codec::StreamingText;
use server_fn::codec::TextStream;
use terrazzo::server;

use crate::api::client_address::ClientAddress;
use crate::text_editor::file_path::FilePath;
use crate::text_editor::fsio::CursorPosition;

#[server(protocol = Http<Json, Json>)]
pub async fn get_highlight_ranges(
    remote: ClientAddress,
    path: FilePath<Arc<Path>>,
    input: String,
) -> Result<Vec<CursorPosition>, ServerFnError> {
    super::service::get_highlight_ranges(remote, path, input).await
}

#[server(protocol = Http<Json, StreamingText>)]
pub async fn search(
    remote: ClientAddress,
    base: Arc<Path>,
    input: String,
) -> Result<TextStream, ServerFnError> {
    use tracing::info_span;
    use tracing_futures::Instrument as _;
    let span = info_span!("Search", ?base, ?input);
    super::service::search(remote, base, input)
        .instrument(span)
        .await
}
