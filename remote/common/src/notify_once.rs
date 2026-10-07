use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;

use futures::FutureExt;
use futures::future::Shared;
use pin_project::pin_project;
use tokio::sync::oneshot;

#[derive(Clone)]
pub struct Sender<T: Clone>(Arc<Mutex<Option<tokio::sync::oneshot::Sender<T>>>>);

#[pin_project]
#[derive(Clone)]
pub struct Receiver<T: Clone>(#[pin] Shared<tokio::sync::oneshot::Receiver<T>>);

pub fn channel<T: Clone>() -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = oneshot::channel();
    (
        Sender(Arc::new(Mutex::new(Some(tx)))),
        Receiver(rx.shared()),
    )
}

impl<T: Clone> Sender<T> {
    pub fn send(&self, value: T) -> Result<(), T> {
        if let Ok(mut lock) = self.0.lock()
            && let Some(tx) = lock.take()
        {
            return tx.send(value);
        }
        return Err(value);
    }
}

impl<T: Clone> Future for Receiver<T> {
    type Output = Result<T, tokio::sync::oneshot::error::RecvError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.project().0.poll(cx)
    }
}

impl<T: Clone + Default> Receiver<T> {
    pub async fn or_default(self) -> T {
        self.await.unwrap_or_default()
    }
}
