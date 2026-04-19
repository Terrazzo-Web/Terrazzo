//! Implementation of the Converter service through the gRPC tunnel.

use tonic::Request;
use tonic::Response;
use tonic::Status;
use tonic::async_trait;

use super::dispatch::conversions_dispatch;
use super::response::remote::RemoteResponseStream;
use crate::backend::client_service::ClientServiceImpl;
use crate::backend::protos::terrazzo::converter::ConversionsRequest;
use crate::backend::protos::terrazzo::converter::converter_service_server::ConverterService;

#[async_trait]
impl ConverterService for ClientServiceImpl {
    type StreamConversionsStream = RemoteResponseStream;

    async fn stream_conversions(
        &self,
        request: Request<ConversionsRequest>,
    ) -> Result<Response<Self::StreamConversionsStream>, Status> {
        let response = conversions_dispatch(request.into_inner())
            .await
            .map_err(Status::from)?;
        Ok(Response::new(RemoteResponseStream(response)))
    }
}
