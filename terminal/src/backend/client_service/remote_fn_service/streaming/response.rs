use crate::backend::protos::terrazzo::remotefn::ServerFnResponse as ServerFnResponseProto;
use pin_project::pin_project;
use server_fn::BoxedStream;
use server_fn::ServerFnError;

pub mod local;
pub mod remote;

#[cfg(debug_assertions)]
type RemoteStream = std::pin::Pin<
    Box<dyn futures::Stream<Item = Result<ServerFnResponseProto, tonic::Status>> + Send + Sync>,
>;

#[cfg(not(debug_assertions))]
type RemoteStream = Box<tonic::Streaming<ServerFnResponseProto>>;

#[pin_project(project = HybridResponseStreamProj)]
pub enum HybridResponseStream {
    Local(BoxedStream<String, ServerFnError>),
    Remote(#[pin] RemoteStream),
}

impl From<HybridResponseStream> for BoxedStream<String, ServerFnError> {
    fn from(response_stream: HybridResponseStream) -> Self {
        match response_stream {
            HybridResponseStream::Local(local_stream) => local_stream,
            remote_stream => self::local::LocalResponseStream(remote_stream).into(),
        }
    }
}
