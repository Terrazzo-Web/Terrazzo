use std::collections::HashMap;

use connection_id::ConnectionId;
use dashmap::DashMap;
use tonic::transport::Channel;

use crate::server::ClientId;

pub mod connection_id;

#[derive(Default)]
pub struct Connections {
    cache: DashMap<ClientId, HashMap<ConnectionId, Channel>>,
}

impl Connections {
    pub fn add(&self, client_id: ClientId, channel: Channel) {
        let connection_id = ConnectionId::next();
        match self.cache.entry(client_id) {
            dashmap::Entry::Occupied(mut entry) => {
                add_channel(entry.get_mut(), connection_id, channel);
            }
            dashmap::Entry::Vacant(entry) => {
                let mut connections = HashMap::new();
                add_channel(&mut connections, connection_id, channel);
                entry.insert(connections);
            }
        }
    }
}

fn add_channel(
    connections: &mut HashMap<ConnectionId, Channel>,
    connection_id: ConnectionId,
    channel: Channel,
) {
    connections.insert(connection_id, channel);
}
