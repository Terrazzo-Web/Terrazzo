use std::sync::Arc;

use server_fn::BoxedStream;
use server_fn::ServerFnError;
use tonic::Status;
use tonic::body::Body as BoxBody;
use tonic::client::GrpcService;
use tonic::codegen::Bytes;
use tonic::codegen::StdError;
use tonic::transport::Body;
use trz_gateway_server::server::Server;

use super::response::HybridResponseStream;
use crate::backend::client_service::routing::DistributedCallback;
use crate::backend::protos::terrazzo::converter::ConversionsRequest;
use crate::backend::protos::terrazzo::converter::converter_service_client::ConverterServiceClient;
use crate::backend::protos::terrazzo::shared::ClientAddress as ClientAddressProto;
use crate::converter::api::Conversion;
use crate::converter::service::stream_conversions;

pub struct ConverterCallback;

impl DistributedCallback for ConverterCallback {
    type Request = ConversionsRequest;
    type Response = HybridResponseStream;
    type LocalError = ConverterLocalError;
    type RemoteError = Status;

    async fn local(
        _server: Option<&Arc<Server>>,
        request: ConversionsRequest,
    ) -> Result<HybridResponseStream, ConverterLocalError> {
        local_conversions_stream(request.input.into())
            .map(HybridResponseStream::Local)
            .map_err(ConverterLocalError)
    }

    async fn remote<T>(
        channel: T,
        client_address: &[impl AsRef<str>],
        mut request: ConversionsRequest,
    ) -> Result<HybridResponseStream, Status>
    where
        T: GrpcService<BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        request.address = Some(ClientAddressProto::of(client_address));
        let mut client = ConverterServiceClient::new(channel);
        let response = client.stream_conversions(request).await?.into_inner();
        Ok(HybridResponseStream::Remote(Box::new(response)))
    }
}

fn local_conversions_stream(
    input: Arc<str>,
) -> Result<BoxedStream<Conversion, ServerFnError>, ServerFnError> {
    Ok(stream_conversions(input))
}

#[derive(thiserror::Error, Debug)]
#[error("{0}")]
pub struct ConverterLocalError(ServerFnError);
