use std::sync::Arc;

use futures::TryStreamExt;
use server_fn::ServerFnError;
use tonic::Status;
use tonic::body::Body as BoxBody;
use tonic::client::GrpcService;
use tonic::codegen::Bytes;
use tonic::codegen::StdError;
use tonic::transport::Body;
use tracing::debug;
use trz_gateway_server::server::Server;

use super::REMOTE_FNS;
use super::response::HybridResponseStream;
use crate::backend::client_service::remote_streaming_fn_service::RemoteFnError;
use crate::backend::client_service::routing::DistributedCallback;
use crate::backend::protos::terrazzo::remotefn::RemoteFnRequest;
use crate::backend::protos::terrazzo::remotefn::remote_streaming_fn_service_client::RemoteStreamingFnServiceClient;
use crate::backend::protos::terrazzo::shared::ClientAddress as ClientAddressProto;

pub struct DistributedFn;

impl DistributedCallback for DistributedFn {
    type Request = RemoteFnRequest;
    type Response = HybridResponseStream;
    type LocalError = RemoteFnError;
    type RemoteError = Status;

    async fn local(
        server: Option<&Arc<Server>>,
        request: RemoteFnRequest,
    ) -> Result<HybridResponseStream, RemoteFnError> {
        debug!("Calling local {request:?}");
        let server = server.ok_or(RemoteFnError::ServerNotSet)?;
        let remote_server_fn = {
            let remote_server_fns = REMOTE_FNS.get().ok_or(RemoteFnError::RemoteFnsNotSet)?;
            remote_server_fns
                .get(request.server_fn_name.as_str())
                .ok_or_else(|| RemoteFnError::RemoteFnNotFound(request.server_fn_name))?
        };
        let callback = &remote_server_fn.callback;
        let local_stream = callback(server, &request.json)
            .map_err(|error| ServerFnError::from(error))
            .into();
        return Ok(HybridResponseStream::Local(local_stream));
    }

    async fn remote<T>(
        channel: T,
        client_address: &[impl AsRef<str>],
        mut request: RemoteFnRequest,
    ) -> Result<HybridResponseStream, Status>
    where
        T: GrpcService<BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        request.address = Some(ClientAddressProto::of(client_address));
        debug!("Calling remote {request:?}");
        let mut client = RemoteStreamingFnServiceClient::new(channel);
        let response = client.call_server_fn(request).await?.into_inner();
        Ok(HybridResponseStream::Remote(Box::new(response)))
    }
}
