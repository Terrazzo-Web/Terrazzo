use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::ready;

use pin_project::pin_project;
use server_fn::ServerFnError;
use tonic::Status;

use super::HybridResponseStream;
use super::HybridResponseStreamProj;
use crate::backend::protos::terrazzo::converter::ConversionResponse as ConversionResponseProto;
use crate::converter::api::Conversion;

#[pin_project(project = RemoteResponseStreamProj)]
pub struct RemoteResponseStream(#[pin] pub HybridResponseStream);

impl futures::Stream for RemoteResponseStream {
    type Item = Result<ConversionResponseProto, Status>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.project().0.project() {
            HybridResponseStreamProj::Local(this) => {
                poll_next_remote(ready!(this.as_mut().poll_next(cx))).into()
            }
            HybridResponseStreamProj::Remote(this) => this.poll_next(cx),
        }
    }
}

fn poll_next_remote(
    response: Option<Result<Conversion, ServerFnError>>,
) -> Option<Result<ConversionResponseProto, Status>> {
    Some(poll_next_remote_some(response?))
}

fn poll_next_remote_some(
    response: Result<Conversion, ServerFnError>,
) -> Result<ConversionResponseProto, Status> {
    let response = response.map_err(|error| Status::internal(format!("Remote error: {error}")))?;
    Ok(ConversionResponseProto {
        language: response.language.name.to_string(),
        content: response.content.clone(),
    })
}
