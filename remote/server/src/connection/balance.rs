use std::convert::Infallible;
use std::marker::Send;

use axum::http;
use bytes::Bytes;
use futures::SinkExt;
use futures::channel::mpsc;
use futures::future::MapErr;
use tonic::body::BoxBody;
use tonic::client::GrpcService;
use tonic::transport::Body;
use tonic::transport::channel::ResponseFuture;
use tower::BoxError;
use tower::Service;
use tower::balance::p2c::Balance;
use tower::buffer::Buffer;
use tower::discover::Change;
use tower::load::CompleteOnResponse;
use tower::load::PendingRequests;
use tower::load::completion::TrackCompletionFuture;
use tower::load::pending_requests::Handle;
use trz_gateway_common::is_global::IsGlobal;

use super::connection_id::ConnectionId;

pub struct IncomingClients<S: Service<http::Request<BoxBody>>> {
    tx: mpsc::UnboundedSender<Result<Change<ConnectionId, PendingRequests<S>>, Infallible>>,
    balanced_channel: Buffer<
        http::Request<BoxBody>,
        MapErr<
            TrackCompletionFuture<ResponseFuture, CompleteOnResponse, Handle>,
            fn(tonic::transport::Error) -> BoxError,
        >,
    >,
}

impl<S> IncomingClients<S>
where
    S: Service<
            http::Request<BoxBody>,
            Response = http::Response<BoxBody>,
            Future = ResponseFuture,
            Error = tonic::transport::Error,
        > + IsGlobal,
{
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded();
        let balanced_channel = Buffer::new(Balance::new(rx), 1);
        Self {
            tx,
            balanced_channel,
        }
    }

    pub fn add_channel(&self, connection_id: ConnectionId, channel: S) {
        self.send_change(Change::Insert(
            connection_id,
            PendingRequests::new(channel.into(), CompleteOnResponse::default()),
        ));
    }

    pub fn remove_channel(&self, connection_id: ConnectionId) {
        self.send_change(Change::Remove(connection_id));
    }

    fn send_change(&self, change: Change<ConnectionId, PendingRequests<S>>) {
        let mut tx = self.tx.clone();
        let _handle = tokio::spawn(async move {
            let _sent = tx.send(Ok(change)).await;
        });
    }

    pub fn get_channel(
        &self,
    ) -> impl GrpcService<
        BoxBody,
        ResponseBody = impl Body<Data = Bytes, Error = impl Into<BoxError> + Send + use<S>> + use<S>,
    > + use<S> {
        self.balanced_channel.clone()
    }
}
