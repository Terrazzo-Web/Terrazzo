use std::future::ready;
use std::path::Path;
use std::sync::Arc;

use futures::Stream;
use futures::StreamExt;
use futures::TryStreamExt as _;
use server_fn::ServerFnError;
use server_fn::codec::TextStream;
use tonic::Status;
use tracing::debug;
use tracing::warn;

use crate::api::client_address::ClientAddress;
use crate::backend::client_service::remote_fn_service;
use crate::text_editor::fsio::FileMetadata;
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
    let results = vec![
        Ok(FileMetadata {
            name: base
                .as_ref()
                .join(input.clone())
                .display()
                .to_string()
                .into(),
            ..FileMetadata::default()
        }),
        Ok(FileMetadata {
            name: base.as_ref().join(input).display().to_string().into(),
            ..FileMetadata::default()
        }),
    ];
    futures::stream::iter(results)
}
