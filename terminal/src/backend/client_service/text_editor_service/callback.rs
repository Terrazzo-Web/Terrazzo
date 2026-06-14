use std::future::ready;
use std::marker::PhantomData;
use std::sync::Arc;

use futures::Stream;
use futures::StreamExt as _;
use futures::TryStreamExt as _;
use futures::stream;
use prost::bytes::Bytes;
use scopeguard::defer;
use tonic::body::Body as BoxBody;
use tonic::client::GrpcService;
use tonic::codegen::StdError;
use tonic::transport::Body;
use tracing::Instrument;
use tracing::debug;
use tracing::debug_span;

use super::DownloadStream;
use super::TextEditorFsioError;
use super::download_local;
use super::upload_local;
use crate::backend::Server;
use crate::backend::client_service::routing::DistributedCallback;
use crate::backend::client_service::routing::DistributedCallbackError;
use crate::backend::protos::terrazzo::shared::ClientAddress as ClientAddressProto;
use crate::backend::protos::terrazzo::texteditor::DownloadRequest;
use crate::backend::protos::terrazzo::texteditor::UploadRequest;
use crate::backend::protos::terrazzo::texteditor::text_editor_service_client::TextEditorServiceClient;
use crate::backend::protos::terrazzo::texteditor::upload_request;
use crate::text_editor::file_path::FilePath;

pub struct DownloadCallback;

impl DownloadCallback {
    pub async fn download(
        server: &Arc<Server>,
        client_address: &[impl AsRef<str>],
        path: FilePath<std::path::PathBuf>,
    ) -> Result<DownloadStream, TextEditorFsioError> {
        Self::process(server, client_address, path)
            .await
            .map_err(map_distributed_error)
    }
}

impl DistributedCallback for DownloadCallback {
    type Request = FilePath<std::path::PathBuf>;
    type Response = DownloadStream;
    type LocalError = TextEditorFsioError;
    type RemoteError = tonic::Status;

    async fn local(
        _server: Option<&Arc<Server>>,
        path: Self::Request,
    ) -> Result<Self::Response, Self::LocalError> {
        debug!("Downloading file {path:?}");
        download_local(path).await
    }

    async fn remote<T>(
        channel: T,
        client_address: &[impl AsRef<str>],
        path: Self::Request,
    ) -> Result<Self::Response, Self::RemoteError>
    where
        T: GrpcService<BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = tonic::codegen::Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        let address = ClientAddressProto::of(client_address);
        let span = debug_span!("Downloading file", ?path, ?address);
        async move {
            debug!("Start");
            defer!(debug!("Done"));
            remote_download_impl(channel, address, path).await
        }
        .instrument(span)
        .await
    }
}

async fn remote_download_impl<T>(
    channel: T,
    address: ClientAddressProto,
    path: FilePath<std::path::PathBuf>,
) -> Result<DownloadStream, tonic::Status>
where
    T: GrpcService<BoxBody>,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = tonic::codegen::Bytes> + Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + Send,
{
    let mut client = TextEditorServiceClient::new(channel);
    let response = client
        .download(DownloadRequest {
            address: Some(address),
            path: Some(path.into()),
        })
        .await?
        .into_inner()
        .map_ok(|response| response.data)
        .map_err(TextEditorFsioError::from);
    Ok(Box::pin(response))
}

pub struct UploadCallback<S>(PhantomData<S>);

impl<S> UploadCallback<S>
where
    S: Stream<Item = Result<Bytes, TextEditorFsioError>> + Send + 'static,
{
    pub async fn upload(
        server: &Arc<Server>,
        client_address: &[impl AsRef<str>],
        path: FilePath<std::path::PathBuf>,
        content: S,
    ) -> Result<(), TextEditorFsioError> {
        Self::process(server, client_address, (path, content))
            .await
            .map_err(map_distributed_error)
    }
}

impl<S> DistributedCallback for UploadCallback<S>
where
    S: Stream<Item = Result<Bytes, TextEditorFsioError>> + Send + 'static,
{
    type Request = (FilePath<std::path::PathBuf>, S);
    type Response = ();
    type LocalError = TextEditorFsioError;
    type RemoteError = tonic::Status;

    async fn local(
        _server: Option<&Arc<Server>>,
        (path, content): Self::Request,
    ) -> Result<Self::Response, Self::LocalError> {
        debug!("Uploading file {path:?}");
        upload_local(path, content).await
    }

    async fn remote<T>(
        channel: T,
        client_address: &[impl AsRef<str>],
        (path, content): Self::Request,
    ) -> Result<Self::Response, Self::RemoteError>
    where
        T: GrpcService<BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = tonic::codegen::Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        let address = ClientAddressProto::of(client_address);
        let span = debug_span!("Downlaoding file", ?path, ?address);
        async move {
            debug!("Start");
            defer!(debug!("Done"));
            remote_upload_impl(channel, address, (path, content)).await
        }
        .instrument(span)
        .await
    }
}

async fn remote_upload_impl<S, T>(
    channel: T,
    address: ClientAddressProto,
    (path, content): (FilePath<std::path::PathBuf>, S),
) -> Result<(), tonic::Status>
where
    S: Stream<Item = Result<Bytes, TextEditorFsioError>> + Send + 'static,
    T: GrpcService<BoxBody>,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = tonic::codegen::Bytes> + Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + Send,
{
    let first = UploadRequest {
        kind: Some(upload_request::Kind::Address(address)),
    };
    let second = UploadRequest {
        kind: Some(upload_request::Kind::Path(path.into())),
    };
    let content = content.map_ok(|data| UploadRequest {
        kind: Some(upload_request::Kind::Data(data)),
    });
    let content = stream::iter([Ok(first), Ok(second)])
        .chain(content)
        .inspect_err(super::warn_stream_error)
        .filter_map(|request| ready(request.ok()));
    let mut client = TextEditorServiceClient::new(channel);
    client.upload(content).await?;
    Ok(())
}

fn map_distributed_error(
    error: DistributedCallbackError<TextEditorFsioError, tonic::Status>,
) -> TextEditorFsioError {
    match error {
        DistributedCallbackError::RemoteError(error) => error.into(),
        DistributedCallbackError::LocalError(error) => error,
        DistributedCallbackError::RemoteClientNotFound(client) => {
            TextEditorFsioError::RemoteClientNotFound(client)
        }
        DistributedCallbackError::ServerNotSet => TextEditorFsioError::ServerNotSet,
    }
}
