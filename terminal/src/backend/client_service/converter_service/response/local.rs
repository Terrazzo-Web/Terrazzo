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
use crate::converter::api::Language;

#[pin_project(project = LocalResponseStreamProj)]
pub struct LocalResponseStream(#[pin] pub HybridResponseStream);

impl futures::Stream for LocalResponseStream {
    type Item = Result<Conversion, ServerFnError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.project().0.project() {
            HybridResponseStreamProj::Local(this) => this.as_mut().poll_next(cx),
            HybridResponseStreamProj::Remote(this) => {
                poll_next_local(ready!(this.poll_next(cx))).into()
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

fn poll_next_local(
    response: Option<Result<ConversionResponseProto, Status>>,
) -> Option<Result<Conversion, ServerFnError>> {
    Some(poll_next_local_some(response?))
}

fn poll_next_local_some(
    response: Result<ConversionResponseProto, Status>,
) -> Result<Conversion, ServerFnError> {
    let response = response?;
    Ok(Conversion::new(
        Language::new(response.language),
        response.content,
    ))
}
