use std::fmt::Debug;
use std::sync::Arc;

use futures::FutureExt;
use futures::channel::oneshot;
use futures::future::Shared;
use std::sync::Mutex;

pub struct WatchTx<T = ()>(Arc<std::sync::Mutex<WatchInner<T>>>);
pub struct WatchRx<T = ()>(Arc<std::sync::Mutex<WatchInner<T>>>);

struct WatchInner<T> {
    sender: oneshot::Sender<T>,
    receiver: Shared<oneshot::Receiver<T>>,
    tx_dropped: bool,
}

impl<T: Clone + Debug> WatchTx<T> {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(WatchInner::new())))
    }

    pub fn notify(&self, value: T) -> Result<(), T> {
        if Arc::strong_count(&self.0) == 1 {
            return Err(value);
        }
        let sender = {
            let mut lock = self.0.lock().expect("Watch lock");
            let old = std::mem::replace(&mut *lock, WatchInner::new());
            old.sender
        };
        let _ = sender.send(value);
        Ok(())
    }

    pub fn subscribe(&self) -> WatchRx<T> {
        WatchRx(self.0.clone())
    }
}

impl<T> Drop for WatchTx<T> {
    fn drop(&mut self) {
        let mut lock = self.0.lock().expect("Watch lock");
        lock.tx_dropped = true;
    }
}

impl<T: Clone> WatchRx<T> {
    pub fn notified(&self) -> impl Future<Output = Result<T, oneshot::Canceled>> {
        let lock = self.0.lock().expect("Watch lock");
        if lock.tx_dropped {
            let (_, rx) = oneshot::channel();
            return rx.shared();
        }
        lock.receiver.clone()
    }
}

impl<T> WatchInner<T>
where
    T: Clone,
{
    fn new() -> Self {
        let (sender, receiver) = oneshot::channel();
        Self {
            sender,
            receiver: receiver.shared(),
            tx_dropped: false,
        }
    }
}

impl<T> Clone for WatchRx<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures::channel::oneshot;
    use tokio::time::error::Elapsed;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_watch() {
        let watch = super::WatchTx::new();
        let rx = watch.subscribe();
        assert!(matches!(
            timeout(Duration::from_millis(100), rx.notified()).await,
            Err(Elapsed { .. }),
        ));
        assert!(watch.notify(1).is_ok());
        assert!(watch.notify(2).is_ok());
        let rx2 = watch.subscribe();
        let rx_fut = rx.notified();
        let rx2_fut = rx2.notified();
        assert!(watch.notify(3).is_ok());
        assert_eq!(Ok(3), rx_fut.await);
        assert_eq!(Ok(3), rx2_fut.await);
        let rx_fut = rx.notified();
        let rx2_fut = rx2.notified();
        assert!(watch.notify(4).is_ok());
        assert_eq!(Ok(4), rx_fut.await);
        assert_eq!(Ok(4), rx2_fut.await);
    }

    #[tokio::test]
    async fn test_watch_drop_tx() {
        let watch = super::WatchTx::new();
        let rx = watch.subscribe();
        assert!(watch.notify(1).is_ok());
        let rx_fut = rx.notified();
        assert!(watch.notify(2).is_ok());
        assert_eq!(Ok(2), rx_fut.await);
        drop(watch);
        assert!(rx.notified().await.is_err());
        assert!(matches!(rx.notified().await, Err(oneshot::Canceled)));
    }

    #[tokio::test]
    async fn test_watch_drop_rx() {
        let watch = super::WatchTx::new();

        assert!(watch.notify(1).is_err());
        assert_eq!(Err(2), watch.notify(2));

        let rx = watch.subscribe();
        let rx_fut = rx.notified();
        assert!(watch.notify(3).is_ok());
        assert_eq!(Ok(3), rx_fut.await);

        drop(rx);
        assert_eq!(Err(4), watch.notify(4));
    }
}
