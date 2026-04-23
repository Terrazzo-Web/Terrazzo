use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::task::ready;

use futures::Stream;
use pin_project::pin_project;
use tonic::Result;
use tonic::Status;
use trz_gateway_server::server::Server;

use super::RemoteFnError;

/// Helper to uplift a remote function into a String -> String server_fn.
pub const fn uplift<Req, S, T, E>(
    function: impl Fn(&Arc<Server>, Req) -> S + 'static,
) -> impl Fn(&Arc<Server>, &str) -> UpliftStream<S>
where
    Req: for<'de> serde::Deserialize<'de>,
    S: Stream<Item = Result<T, E>>,
    T: serde::Serialize,
    Status: From<E>,
{
    move |server, request| {
        let request = serde_json::from_str::<Req>(request)
            .map_err(|error| RemoteFnError::DeserializeRequest(error, request.into()));
        match request {
            Ok(request) => UpliftStream::Stream(function(server, request)),
            Err(error) => UpliftStream::DeserializeRequest(error),
        }
    }
}

#[pin_project(project = UpliftStreamProj)]
pub enum UpliftStream<S> {
    DeserializeRequest(RemoteFnError),
    Stream(#[pin] S),
}

impl<S, T, E> Stream for UpliftStream<S>
where
    S: Stream<Item = Result<T, E>>,
    T: serde::Serialize,
    Status: From<E>,
{
    type Item = Result<String, RemoteFnError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.project() {
            UpliftStreamProj::DeserializeRequest(error) => {
                let error = std::mem::replace(error, RemoteFnError::RemoteFnsNotSet);
                Some(Err(error))
            }
            UpliftStreamProj::Stream(stream) => match ready!(stream.poll_next(cx)) {
                Some(Ok(response)) => {
                    Some(serde_json::to_string(&response).map_err(RemoteFnError::SerializeResponse))
                }
                Some(Err(error)) => Some(Err(RemoteFnError::Status(error.into()))),
                None => None,
            },
        }
        .into()
    }
}
