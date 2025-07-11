//! Cache of client connections to the Terrazzo Gateway.

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use connection_id::ConnectionId;
use dashmap::DashMap;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use scopeguard::defer;
use tokio::time::error::Elapsed;
use tokio::time::timeout;
use tonic::transport::Channel;
use tracing::Instrument as _;
use tracing::info;
use tracing::info_span;
use tracing::warn;
use trz_gateway_common::consts::HEALTH_CHECK_PERIOD;
use trz_gateway_common::consts::HEALTH_CHECK_TIMEOUT;
use trz_gateway_common::id::ClientName;
use trz_gateway_common::protos::terrazzo::remote::health::Ping;
use trz_gateway_common::protos::terrazzo::remote::health::Pong;
use trz_gateway_common::protos::terrazzo::remote::health::health_service_client::HealthServiceClient;

use self::balance::IncomingClients;
use crate::auth_code::AuthCode;

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
                let start = Instant::now();
                let pong = health_client.ping_pong(Ping {
                    connection_id: connection_id.to_string(),
                    ..Ping::default()
                });
                let Pong { .. } = timeout(HEALTH_CHECK_TIMEOUT, pong).await??.into_inner();
                let latency = Instant::now() - start;
                info!(?latency, "Ping");

                let start = Instant::now();
                let pong = health_client.ping_pong(Ping {
                    connection_id: connection_id.to_string(),
                    delay: Some(HEALTH_CHECK_PERIOD.try_into()?),
                    auth_code: AuthCode::current().to_string(),
                });
                let Pong { .. } = timeout(HEALTH_CHECK_PERIOD + HEALTH_CHECK_TIMEOUT, pong)
                    .await??
                    .into_inner();
                let elapsed = start.elapsed();
                if elapsed < HEALTH_CHECK_PERIOD {
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
        let dashmap::Entry::Occupied(mut connections) = self.cache.entry(client_name) else {
            return;
        };
        connections.get_mut().remove_channel(connection_id);
        if connections.get_mut().is_empty() {
            connections.remove();
        }
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ChannelHealthError {
    #[error("[{n}] {0}", n = self.name())]
    GrpcError(#[from] tonic::Status),

    #[error("[{n}] {0}", n = self.name())]
    Timeout(#[from] Elapsed),

    #[error("[{n}] {0}", n = self.name())]
    DurationError(#[from] prost_types::DurationError),

    #[error("[{n}] Client slept for {0:?}, should have been {HEALTH_CHECK_PERIOD:?}", n = self.name())]
    TooSoon(Duration),
}

impl Connections {
    /// Returns the list of connected clients.
    pub fn clients(&self) -> Vec<ClientName> {
        self.cache.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Returns a connection for the given client.
    ///
    /// Multiple connections for the same [ClientName] are load-balanced.
    pub fn get_client(
        &self,
        client_name: &ClientName,
    ) -> Option<pending_requests::PendingRequests<Channel>> {
        self.cache
            .get_mut(client_name)
            .and_then(|mut c| c.get_channel())
    }
}
