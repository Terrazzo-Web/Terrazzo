use std::sync::Arc;

use tokio::sync::mpsc;
use trz_gateway_common::p2p::protocol::P2pConnectionId;
use trz_gateway_common::p2p::protocol::SignalMessage;

use super::registration::Registration;

/// One client connection attempt attached to a specific registration.
pub struct Session {
    pub connection_id: P2pConnectionId,
    pub registration: Arc<Registration>,
    pub incoming: mpsc::Receiver<SignalMessage>,
}
