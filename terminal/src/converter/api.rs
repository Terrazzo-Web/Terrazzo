use std::sync::Arc;

use server_fn::Http;
use server_fn::ServerFnError;
use server_fn::codec::Json;
use server_fn::codec::StreamingText;
use server_fn::codec::TextStream;
use terrazzo::server;

use crate::api::client_address::ClientAddress;

#[server(protocol = Http<Json, StreamingText>)]
pub async fn get_conversions(
    remote: Option<ClientAddress>,
    input: Arc<str>,
) -> Result<TextStream<ServerFnError>, ServerFnError> {
    imp::stream_impl(remote, input).await
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct ConversionsRequest {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "c"))]
    pub input: Arc<str>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct Conversions {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "c"))]
    pub conversions: Arc<Vec<Conversion>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ConversionImpl {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "l"))]
    pub language: Language,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "c"))]
    pub content: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Conversion(Arc<ConversionImpl>);

impl Conversion {
    #[cfg(feature = "server")]
    pub fn new(language: Language, content: String) -> Self {
        Self(Arc::new(ConversionImpl { language, content }))
    }
}

impl std::ops::Deref for Conversion {
    type Target = ConversionImpl;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Debug for Conversion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conversion")
            .field("language", &self.language)
            .finish()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
// serde
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Language {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "n"))]
    pub name: Arc<str>,
}

impl Language {
    #[cfg(feature = "server")]
    pub fn new(name: impl Into<Arc<str>>) -> Self {
        Self { name: name.into() }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.name, f)
    }
}

#[cfg(feature = "server")]
mod imp {
    use std::pin::Pin;
    use std::sync::Arc;

    use futures::Stream;
    use futures::TryStreamExt as _;
    use futures::future;
    use server_fn::BoxedStream;
    use server_fn::ServerFnError;
    use server_fn::codec::TextStream;

    use super::Conversion;
    use crate::api::client_address::ClientAddress;
    use crate::backend::client_service::converter_service::dispatch::conversions_dispatch;
    use crate::backend::protos::terrazzo::converter::ConversionsRequest as ProtoConversionsRequest;
    use crate::backend::protos::terrazzo::shared::ClientAddress as ClientAddressProto;
    use crate::utils::ndjson::serialize_line;

    pub(super) async fn stream_impl(
        remote: Option<ClientAddress>,
        input: Arc<str>,
    ) -> Result<TextStream<ServerFnError>, ServerFnError> {
        let request = ProtoConversionsRequest {
            address: remote.map(|remote| ClientAddressProto::of(&remote)),
            input: input.to_string(),
        };
        let stream = conversions_dispatch(request)
            .await
            .map(BoxedStream::from)
            .map_err(ServerFnError::new)?;
        let stream: Pin<Box<dyn Stream<Item = _> + Send>> = stream.into();
        let stream = stream.and_then(|conversion: Conversion| {
            future::ready(serialize_line(&conversion).map_err(ServerFnError::new))
        });
        Ok(TextStream::new(stream))
    }
}

#[cfg(test)]
#[cfg(feature = "server")]
mod tests {
    use futures::StreamExt as _;

    use super::Conversion;
    use super::get_conversions;
    use crate::utils::ndjson::NdjsonBuffer;

    #[tokio::test]
    async fn get_conversions_streams_ndjson_rows() {
        let stream = get_conversions(None, "abc".into()).await.expect("stream");
        let mut parser = NdjsonBuffer::<Conversion>::default();
        let mut conversions = vec![];
        let mut stream = stream.into_inner();
        while let Some(chunk) = stream.next().await {
            for conversion in parser.push_chunk(&chunk.expect("chunk")) {
                conversions.push(conversion.expect("conversion"));
            }
        }

        assert!(
            conversions
                .iter()
                .any(|conversion| conversion.language.name.as_ref() == "JSON"
                    && conversion.content == "\"abc\""),
            "{conversions:?}"
        );
    }
}
