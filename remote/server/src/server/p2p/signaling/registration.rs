use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;
use tokio::sync::watch;
use trz_gateway_common::p2p::protocol::FailureCode;
use trz_gateway_common::p2p::protocol::P2pConnectionId;
use trz_gateway_common::p2p::protocol::SignalMessage;

use super::MAX_PENDING_SESSIONS;
use super::MAX_SESSIONS_PER_SERVER;
use super::SIGNAL_QUEUE_CAPACITY;
use super::session::Session;

/// One generation of a server's persistent registration WebSocket.
///
/// Sessions are scoped to this generation. Replacing the registration marks it
/// inactive, fails its sessions, and closes its relay without affecting the new
/// generation installed under the same name.
pub struct Registration {
    pub generation: u64,
    pub outgoing: mpsc::Sender<SignalMessage>,
    sessions: Mutex<HashMap<P2pConnectionId, mpsc::Sender<SignalMessage>>>,
    pending_sessions: Arc<AtomicUsize>,
    pub active: AtomicBool,
    pub close: watch::Sender<bool>,
}

impl Registration {
    pub fn new(
        generation: u64,
        outgoing: mpsc::Sender<SignalMessage>,
        pending_sessions: Arc<AtomicUsize>,
        close: watch::Sender<bool>,
    ) -> Self {
        Self {
            generation,
            outgoing,
            sessions: Mutex::new(HashMap::new()),
            pending_sessions,
            active: AtomicBool::new(true),
            close,
        }
    }

    pub fn create_session(self: &Arc<Self>) -> Result<Session, ()> {
        if !self.active.load(Ordering::Acquire) {
            return Err(());
        }
        if self
            .pending_sessions
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_PENDING_SESSIONS).then_some(count + 1)
            })
            .is_err()
        {
            return Err(());
        }
        let connection_id = P2pConnectionId::new();
        let (sender, incoming) = mpsc::channel(SIGNAL_QUEUE_CAPACITY);
        let inserted = {
            let mut sessions = self.sessions.lock().expect("registration sessions");
            if sessions.len() >= MAX_SESSIONS_PER_SERVER || !self.active.load(Ordering::Acquire) {
                false
            } else {
                sessions.insert(connection_id, sender);
                true
            }
        };
        if !inserted {
            self.pending_sessions.fetch_sub(1, Ordering::AcqRel);
            return Err(());
        }
        Ok(Session {
            connection_id,
            registration: self.clone(),
            incoming,
        })
    }

    pub fn relay_to_client(&self, message: SignalMessage) -> Result<(), ()> {
        message.validate().map_err(|_| ())?;
        let connection_id = message.connection_id().ok_or(())?;
        self.sessions
            .lock()
            .expect("registration sessions")
            .get(&connection_id)
            .ok_or(())?
            .try_send(message)
            .map_err(|_| ())
    }

    pub fn remove_session(&self, connection_id: P2pConnectionId) {
        if self
            .sessions
            .lock()
            .expect("registration sessions")
            .remove(&connection_id)
            .is_some()
        {
            self.pending_sessions.fetch_sub(1, Ordering::AcqRel);
        }
    }

    pub fn cancel(&self, code: FailureCode, detail: &str) {
        if !self.active.swap(false, Ordering::AcqRel) {
            return;
        }
        self.close.send_replace(true);
        let sessions = self
            .sessions
            .lock()
            .expect("registration sessions")
            .drain()
            .collect::<Vec<_>>();
        self.pending_sessions
            .fetch_sub(sessions.len(), Ordering::AcqRel);
        for (connection_id, sender) in sessions {
            let _ = sender.try_send(SignalMessage::Failure {
                connection_id,
                code,
                detail: detail.into(),
            });
        }
    }
}
