use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use bytes::Bytes;
use connection_id::ConnectionId;
use dashmap::DashMap;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use scopeguard::defer;
use tokio::time::error::Elapsed;
use tokio::time::timeout;
use tonic::body::BoxBody;
use tonic::client::GrpcService;
use tonic::transport::Body;
use tonic::transport::Channel;
use tower::BoxError;
use tracing::Instrument as _;
use tracing::info;
use tracing::info_span;
use tracing::warn;
use trz_gateway_common::id::ClientName;
use trz_gateway_common::protos::terrazzo::remote::health::Ping;
use trz_gateway_common::protos::terrazzo::remote::health::health_service_client::HealthServiceClient;

use self::balance::IncomingClients;

mod balance;
pub mod connection_id;
mod pending_requests;

/// The cache of all the channels connected to the Terrazzo Gateway,
/// grouped by [ClientName].
#[derive(Default)]
pub struct Connections {
    cache: DashMap<ClientName, IncomingClients<Channel>>,
}

impl Connections {
    /// Adds a connection to the cache and schedules the periodic keep-alive.
    ///
    /// The connection is removed from the cache, and closed, if the keep-alive fails.
    pub fn add(self: &Arc<Self>, client_name: ClientName, channel: Channel) {
        let connection_id = ConnectionId::next();
        let _span = info_span!("Connection", %connection_id).entered();
        match self.cache.entry(client_name.clone()) {
            dashmap::Entry::Occupied(mut entry) => {
                self.add_channel(entry.get_mut(), client_name, connection_id, channel);
            }
            dashmap::Entry::Vacant(entry) => {
                let mut connections = IncomingClients::new();
                self.add_channel(&mut connections, client_name, connection_id, channel);
                entry.insert(connections);
            }
        }
    }

    fn add_channel(
        self: &Arc<Self>,
        connections: &mut IncomingClients<Channel>,
        client_name: ClientName,
        connection_id: ConnectionId,
        channel: Channel,
    ) {
        connections.add_channel(connection_id, channel.clone());
        tokio::spawn(
            self.clone()
                .channel_health_check(client_name, connection_id, channel)
                .in_current_span(),
        );
    }

    /// Runs the keep-alive.
    ///   1. First, run a quick ping/pong check
    ///   2. Then, send a ping and expect a response after [PERIOD]
    ///   3. go back to step 1
    async fn channel_health_check(
        self: Arc<Connections>,
        client_name: ClientName,
        connection_id: ConnectionId,
        channel: Channel,
    ) -> Result<(), ChannelHealthError> {
        defer!(self.remove(client_name, connection_id));
        let mut health_client = HealthServiceClient::new(channel);
        let health_check_loop = async move {
            loop {
                let pong = health_client.ping_pong(Ping {
                    connection_id: connection_id.to_string(),
                    ..Ping::default()
                });
                timeout(TIMEOUT, pong).await??;

                let start = Instant::now();
                let pong = health_client.ping_pong(Ping {
                    connection_id: connection_id.to_string(),
                    delay: Some(PERIOD.try_into()?),
                });
                timeout(PERIOD + TIMEOUT, pong).await??;
                let elapsed = start.elapsed();
                if elapsed < PERIOD {
                    return Err(ChannelHealthError::TooSoon(elapsed));
                }
            }
        };
        health_check_loop
            .await
            .inspect(|()| info!("Health check loop DONE"))
            .inspect_err(|error| warn!("Health check loop FAILED: {error}"))
    }

    fn remove(self: &Arc<Self>, client_name: ClientName, connection_id: ConnectionId) {
        let Some(mut connections) = self.cache.get_mut(&client_name) else {
            return;
        };
        connections.value_mut().remove_channel(connection_id);
    }
}

const TIMEOUT: Duration = if cfg!(debug_assertions) {
    Duration::from_secs(2)
} else {
    Duration::from_secs(5)
};

const PERIOD: Duration = if cfg!(debug_assertions) {
    Duration::from_secs(10)
} else {
    Duration::from_secs(3 * 60 + 45)
};

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ChannelHealthError {
    #[error("[{n}]  {0}", n = self.name())]
    GrpcError(#[from] tonic::Status),

    #[error("[{n}] {0}", n = self.name())]
    Timeout(#[from] Elapsed),

    #[error("[{n}] {0}", n = self.name())]
    DurationError(#[from] prost_types::DurationError),

    #[error("[{n}] Client slept for {0:?}, should have been {PERIOD:?}", n = self.name())]
    TooSoon(Duration),
}

impl Connections {
    pub fn clients(&self) -> impl Iterator<Item = ClientName> + '_ {
        self.cache.iter().map(|entry| entry.key().clone())
    }

    pub fn get_client(
        &self,
        client_name: &ClientName,
    ) -> Option<
        impl GrpcService<
            BoxBody,
            ResponseBody = impl Body<Data = Bytes, Error = impl Into<BoxError> + Send + use<>> + use<>,
        > + use<>,
    > {
        self.cache
            .get_mut(client_name)
            .and_then(|mut c| c.get_channel())
    }
}
