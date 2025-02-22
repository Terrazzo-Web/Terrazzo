use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use bytes::Bytes;
use connection_id::ConnectionId;
use dashmap::DashMap;
use nameth::nameth;
use nameth::NamedEnumValues as _;
use scopeguard::defer;
use tokio::time::error::Elapsed;
use tokio::time::timeout;
use tonic::body::BoxBody;
use tonic::client::GrpcService;
use tonic::transport::Body;
use tonic::transport::Channel;
use tower::BoxError;
use tracing::Instrument;
use trz_gateway_common::id::ClientId;
use trz_gateway_common::protos::terrazzo::remote::health::health_service_client::HealthServiceClient;
use trz_gateway_common::protos::terrazzo::remote::health::Ping;

use self::balance::IncomingClients;

mod balance;
pub mod connection_id;

#[derive(Default)]
pub struct Connections {
    cache: DashMap<ClientId, IncomingClients<Channel>>,
}

impl Connections {
    pub fn add(self: &Arc<Self>, client_id: ClientId, channel: Channel) {
        let connection_id = ConnectionId::next();
        match self.cache.entry(client_id.clone()) {
            dashmap::Entry::Occupied(mut entry) => {
                self.add_channel(entry.get_mut(), client_id, connection_id, channel);
            }
            dashmap::Entry::Vacant(entry) => {
                let mut connections = IncomingClients::new();
                self.add_channel(&mut connections, client_id, connection_id, channel);
                entry.insert(connections);
            }
        }
    }

    fn add_channel(
        self: &Arc<Self>,
        connections: &mut IncomingClients<Channel>,
        client_id: ClientId,
        connection_id: ConnectionId,
        channel: Channel,
    ) {
        connections.add_channel(connection_id, channel.clone());
        tokio::spawn(
            self.clone()
                .channel_health_check(client_id, connection_id, channel)
                .in_current_span(),
        );
    }

    async fn channel_health_check(
        self: Arc<Connections>,
        client_id: ClientId,
        connection_id: ConnectionId,
        channel: Channel,
    ) -> Result<(), ChannelHealthError> {
        defer!(self.remove(client_id, connection_id));
        let mut health_client = HealthServiceClient::new(channel);
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
    }

    fn remove(self: &Arc<Self>, client_id: ClientId, connection_id: ConnectionId) {
        let Some(connections) = self.cache.get(&client_id) else {
            return;
        };
        connections.value().remove_channel(connection_id);
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
    pub fn client_ids(&self) -> impl Iterator<Item = ClientId> + '_ {
        self.cache.iter().map(|entry| entry.key().clone())
    }

    pub fn get_client(
        &self,
        client_id: &ClientId,
    ) -> Option<
        impl GrpcService<
            BoxBody,
            ResponseBody = impl Body<Data = Bytes, Error = impl Into<BoxError> + Send>,
        >,
    > {
        self.cache.get(client_id).map(|c| c.get_channel().clone())
    }
}
