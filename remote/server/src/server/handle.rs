use nameth::nameth;
use nameth::NamedEnumValues as _;
use tokio::sync::oneshot;
use tracing::warn;

#[must_use]
pub struct ServerHandle {
    shutdown_tx: Option<oneshot::Sender<String>>,
    terminated_rx: Option<oneshot::Receiver<()>>,
}

impl ServerHandle {
    pub fn new() -> (oneshot::Receiver<String>, oneshot::Sender<()>, Self) {
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (terminated_tx, terminated_rx) = oneshot::channel();
        let handle = Self {
            shutdown_tx: Some(shutdown_tx),
            terminated_rx: Some(terminated_rx),
        };
        (shutdown_rx, terminated_tx, handle)
    }

    pub async fn stop(mut self, reason: impl std::fmt::Display) -> Result<(), ServerStopError> {
        self.shutdown_tx
            .take()
            .expect("shutdown_tx")
            .send(format!("{reason}"))
            .map_err(|_| ServerStopError::NotRunning)?;
        let () = self
            .terminated_rx
            .take()
            .expect("terminated_rx")
            .await
            .map_err(|_| ServerStopError::ShutdownError)?;
        Ok(())
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        if (self.shutdown_tx.is_some() || self.terminated_rx.is_some()) && !std::thread::panicking()
        {
            warn!("The server was not shutdown");
            debug_assert!(false);
        }
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ServerStopError {
    #[error("[{n}] The server was not running", n = self.name())]
    NotRunning,

    #[error("[{n}] The server did not fully shutdown", n = self.name())]
    ShutdownError,
}
