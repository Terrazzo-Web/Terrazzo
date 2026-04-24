use pin_project::pin_project;
use server_fn::BoxedStream;
use server_fn::ServerFnError;
use tonic::Streaming;

use crate::backend::protos::terrazzo::converter::ConversionResponse as ConversionResponseProto;
use crate::converter::api::Conversion;

pub mod local;
pub mod remote;

#[pin_project(project = HybridResponseStreamProj)]
pub enum HybridResponseStream {
    Local(BoxedStream<Conversion, ServerFnError>),
    Remote(#[pin] Box<Streaming<ConversionResponseProto>>),
}

impl From<HybridResponseStream> for BoxedStream<Conversion, ServerFnError> {
    fn from(response_stream: HybridResponseStream) -> Self {
        match response_stream {
            HybridResponseStream::Local(local_stream) => local_stream,
            response_stream => self::local::LocalResponseStream(response_stream).into(),
        }
    }
}
