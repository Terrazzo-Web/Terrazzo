use std::future::Future;

use nameth::nameth;
use nameth::NamedEnumValues as _;
use tokio::sync::oneshot;
use tracing::info;
use tracing::warn;

#[must_use]
pub struct ServerHandle<R> {
    shutdown_tx: Option<oneshot::Sender<String>>,
    terminated_rx: Option<oneshot::Receiver<R>>,
}

impl<R> ServerHandle<R> {
    pub fn new() -> (impl Future<Output = ()>, oneshot::Sender<R>, Self) {
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let shutdown_rx = async move {
            match shutdown_rx.await {
                Ok(message) => info!("Server shutdown: {message}"),
                Err(oneshot::error::RecvError { .. }) => warn!("Server handle dropped!"),
            }
        };
        let (terminated_tx, terminated_rx) = oneshot::channel();
        let handle = Self {
            shutdown_tx: Some(shutdown_tx),
            terminated_rx: Some(terminated_rx),
        };
        (shutdown_rx, terminated_tx, handle)
    }

    pub async fn stop(mut self, reason: impl std::fmt::Display) -> Result<R, ServerStopError> {
        self.shutdown_tx
            .take()
            .expect("shutdown_tx")
            .send(format!("{reason}"))
            .map_err(|_| ServerStopError::NotRunning)?;
        self.stopped().await
    }

    pub async fn stopped(mut self) -> Result<R, ServerStopError> {
        self.terminated_rx
            .take()
            .expect("terminated_rx")
            .await
            .map_err(|_| ServerStopError::ShutdownError)
    }
}

impl<R> Drop for ServerHandle<R> {
    fn drop(&mut self) {
        if (self.shutdown_tx.is_some() || self.terminated_rx.is_some()) && !std::thread::panicking()
        {
            warn!("The server was not shutdown");
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
