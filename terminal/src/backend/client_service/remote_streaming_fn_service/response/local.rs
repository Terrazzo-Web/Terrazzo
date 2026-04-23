use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::ready;

use pin_project::pin_project;
use server_fn::ServerFnError;
use tonic::Status;

use super::HybridResponseStream;
use super::HybridResponseStreamProj;
use crate::backend::protos::terrazzo::remotefn::ServerFnResponse as ServerFnResponseProto;

#[pin_project(project = LocalResponseStreamProj)]
pub struct LocalResponseStream(#[pin] pub HybridResponseStream);

impl futures::Stream for LocalResponseStream {
    type Item = Result<String, ServerFnError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.project().0.project() {
            HybridResponseStreamProj::Local(this) => this.as_mut().poll_next(cx),
            HybridResponseStreamProj::Remote(this) => {
                poll_next_local(ready!(this.poll_next(cx))).into()
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.0 {
            HybridResponseStream::Local(local_stream) => local_stream.size_hint(),
            HybridResponseStream::Remote(remote_stream) => remote_stream.size_hint(),
        }
    }
}

fn poll_next_local(
    response: Option<Result<ServerFnResponseProto, Status>>,
) -> Option<Result<String, ServerFnError>> {
    Some(poll_next_local_some(response?))
}

fn poll_next_local_some(
    response: Result<ServerFnResponseProto, Status>,
) -> Result<String, ServerFnError> {
    let response = response?;
    Ok(response.json)
}
