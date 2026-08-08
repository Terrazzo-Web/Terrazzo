use std::path::Path;
use std::sync::Arc;

use futures::Stream;
use futures::StreamExt as _;
use server_fn::ServerFnError;

use crate::api::client_address::ClientAddress;
use crate::text_editor::fsio::FileMetadata;
use crate::utils::ndjson::NdjsonBuffer;

pub async fn search(
    remote: ClientAddress,
    base: Arc<Path>,
    input: String,
) -> Result<impl Stream<Item = Result<FileMetadata, ServerFnError>>, ServerFnError> {
    let stream = super::api::search(remote, base, input).await?.into_inner();
    let mut parser = NdjsonBuffer::<FileMetadata>::default();
    Ok(stream.flat_map(move |item| match item {
        Ok(chunk) => {
            let messages = parser.push_chunk(&chunk);
            futures::stream::iter(
                messages
                    .into_iter()
                    .map(|row| row.map_err(ServerFnError::from))
                    .collect::<Vec<_>>(),
            )
        }
        Err(error) => futures::stream::iter(vec![Err(error)]),
    }))
}
