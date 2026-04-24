use tonic::async_trait;

use super::dispatch::remote_fn_dispatch;
use super::response::remote::RemoteResponseStream;
use crate::backend::client_service::ClientServiceImpl;
use crate::backend::protos::terrazzo::remotefn::RemoteFnRequest;
use crate::backend::protos::terrazzo::remotefn::remote_streaming_fn_service_server::RemoteStreamingFnService;

#[async_trait]
impl RemoteStreamingFnService for ClientServiceImpl {
    type CallServerFnStream = super::response::remote::RemoteResponseStream;

    async fn call_server_fn(
        &self,
        request: tonic::Request<RemoteFnRequest>,
    ) -> Result<tonic::Response<Self::CallServerFnStream>, tonic::Status> {
        let mut request = request.into_inner();
        let address = request.address.get_or_insert_default();
        let address = std::mem::take(&mut address.via);
        let response = remote_fn_dispatch(&self.server, &address, request).await?;
        Ok(tonic::Response::new(RemoteResponseStream(response)))
    }
}
