use futures::StreamExt as _;
use futures::TryStreamExt as _;
use tonic::Request;
use tonic::Response;
use tonic::Streaming;
use tonic::async_trait;

use crate::backend::client_service::ClientServiceImpl;
use crate::backend::client_service::text_editor_service::TextEditorFsioError;
use crate::backend::client_service::text_editor_service::download;
use crate::backend::client_service::text_editor_service::upload;
use crate::backend::protos::terrazzo::shared::Empty;
use crate::backend::protos::terrazzo::texteditor::DownloadRequest;
use crate::backend::protos::terrazzo::texteditor::DownloadResponse;
use crate::backend::protos::terrazzo::texteditor::UploadRequest;
use crate::backend::protos::terrazzo::texteditor::text_editor_service_server::TextEditorService;
use crate::backend::protos::terrazzo::texteditor::upload_request;

#[async_trait]
impl TextEditorService for ClientServiceImpl {
    type DownloadStream =
        futures::stream::BoxStream<'static, Result<DownloadResponse, tonic::Status>>;

    async fn download(
        &self,
        request: Request<DownloadRequest>,
    ) -> Result<Response<Self::DownloadStream>, tonic::Status> {
        let DownloadRequest { address, path } = request.into_inner();
        let address = address.ok_or(TextEditorFsioError::MissingAddress)?;
        let path = path.ok_or(TextEditorFsioError::MissingPath)?;
        let stream = download(&self.server, &address.via, path.into()).await?;
        let stream = stream
            .inspect_err(crate::backend::client_service::text_editor_service::warn_stream_error)
            .map_ok(|data| DownloadResponse { data })
            .map_err(tonic::Status::from)
            .boxed();
        Ok(Response::new(stream))
    }

    async fn upload(
        &self,
        request: Request<Streaming<UploadRequest>>,
    ) -> Result<Response<Empty>, tonic::Status> {
        let mut request = request.into_inner();
        let first = request
            .next()
            .await
            .ok_or(TextEditorFsioError::MissingAddress)??;
        let address = match first.kind {
            Some(upload_request::Kind::Address(address)) => address,
            _ => return Err(TextEditorFsioError::MissingAddress.into()),
        };
        let second = request
            .next()
            .await
            .ok_or(TextEditorFsioError::MissingPath)??;
        let path = match second.kind {
            Some(upload_request::Kind::Path(path)) => path.into(),
            _ => return Err(TextEditorFsioError::MissingPath.into()),
        };
        let content = request.map(|request| match request {
            Ok(UploadRequest {
                kind: Some(upload_request::Kind::Data(data)),
            }) => Ok(data),
            Ok(_) => Err(TextEditorFsioError::UnexpectedUploadMessage),
            Err(error) => Err(error.into()),
        });
        upload(&self.server, &address.via, path, content).await?;
        Ok(Response::new(Empty {}))
    }
}
