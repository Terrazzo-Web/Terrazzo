use std::marker::Send;

use axum::http;
use bytes::Bytes;
use tonic::body::BoxBody;
use tonic::client::GrpcService;
use tonic::transport::Body;
use tonic::transport::channel::ResponseFuture;
use tower::BoxError;
use tower::Service;
use tower::load::Load as _;
use tower::util::rng::HasherRng;
use tower::util::rng::Rng;
use tracing::info;
use trz_gateway_common::is_global::IsGlobal;

use super::connection_id::ConnectionId;
use super::pending_requests::PendingRequests;

/// A struct that maintains a list of channels for a given client.
///
/// A client can open multiple tunnels with the Terrazzo Gateway.
pub struct IncomingClients<S: Service<http::Request<BoxBody>>> {
    channels: Vec<ChannelWithId<S>>,
    rng: HasherRng,
}

struct ChannelWithId<S> {
    connection_id: ConnectionId,
    channel: PendingRequests<S>,
}

impl<S> IncomingClients<S>
where
    S: Service<
            http::Request<BoxBody>,
            Response = http::Response<BoxBody>,
            Future = ResponseFuture,
            Error = tonic::transport::Error,
        > + Clone
        + IsGlobal,
{
    pub fn new() -> Self {
        Self {
            channels: vec![],
            rng: Default::default(),
        }
    }

    /// Adds a channel
    pub fn add_channel(&mut self, connection_id: ConnectionId, channel: S) {
        info!("Adding channel");
        let channel = PendingRequests::new(channel);
        self.channels.push(ChannelWithId {
            connection_id,
            channel,
        });
    }

    /// Removes a channel
    pub fn remove_channel(&mut self, connection_id: ConnectionId) {
        info!("Removing channel");
        self.channels = std::mem::take(&mut self.channels)
            .into_iter()
            .filter(|c| c.connection_id != connection_id)
            .collect();
    }

    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    /// Returns a channel.
    ///
    /// If a client has ≥ 2 tunnels, the load-balancing algorithm choses
    /// channels that have less load based on the number of running requests.
    pub fn get_channel(
        &mut self,
    ) -> Option<
        impl GrpcService<
            BoxBody,
            ResponseBody = impl Body<Data = Bytes, Error = impl Into<BoxError> + Send + use<S>> + use<S>,
        > + use<S>,
    > {
        let count = self.channels.len();
        if count < 2 {
            return if count == 0 {
                None
            } else {
                Some(self.channels[0].channel.clone())
            };
        }
        let [a, b] =
            sample_floyd2(&mut self.rng, count as u64).map(|i| &self.channels[i as usize].channel);
        Some(if a.load() < b.load() { a } else { b }.clone())
    }
}

fn sample_floyd2<R: Rng>(rng: &mut R, length: u64) -> [u64; 2] {
    debug_assert!(2 <= length);
    let aidx = rng.next_range(0..length - 1);
    let bidx = rng.next_range(0..length);
    let aidx = if aidx == bidx { length - 1 } else { aidx };
    [aidx, bidx]
}
