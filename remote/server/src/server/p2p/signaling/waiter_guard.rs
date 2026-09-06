use std::sync::Arc;

use trz_gateway_common::id::ClientName;

use super::Signaling;

/// Removes a registration waiter from the registry on completion or cancellation.
pub struct WaiterGuard {
    pub signaling: Arc<Signaling>,
    pub server_name: ClientName,
}

impl Drop for WaiterGuard {
    fn drop(&mut self) {
        let mut state = self.signaling.state.lock().expect("signaling state");
        let remove = if let Some(waiters) = state.waiters.get_mut(&self.server_name) {
            waiters.count -= 1;
            waiters.count == 0
        } else {
            false
        };
        if remove {
            state.waiters.remove(&self.server_name);
        }
    }
}
