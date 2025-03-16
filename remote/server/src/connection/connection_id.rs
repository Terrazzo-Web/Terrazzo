use std::sync::atomic::AtomicUsize;

/// A unique identifier for a client connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectionId(usize);

static NEXT_CONNECTION_ID: AtomicUsize = AtomicUsize::new(0);

impl ConnectionId {
    pub fn next() -> Self {
        Self(NEXT_CONNECTION_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
    }
}

impl std::fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
