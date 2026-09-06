use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use std::task::ready;
use std::time::Duration;

use futures::FutureExt as _;
use futures::future::Either;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;

/// Represents a future that resolves when it is time to send a ping.
pub struct Ping {
    /// A future that resolves when it is time to send a Ping
    ping: oneshot::Receiver<Result<(), ()>>,
    ping_ok: Option<oneshot::Sender<()>>,
    timeout: Duration,
}

/// Represents a struct that must be called when receiving a pong
pub struct Pong {
    pong: Arc<Mutex<Option<oneshot::Sender<Result<(), ()>>>>>,
}

impl Ping {
    pub fn new(timeout: Duration) -> (Self, Pong) {
        let (pong, ping) = oneshot::channel();
        let (ping_ok, pong_ok) = oneshot::channel();
        let ping = Self {
            ping,
            ping_ok: Some(ping_ok),
            timeout,
        };
        let pong = Arc::new(Mutex::new(Some(pong)));
        {
            let pong = Pong { pong: pong.clone() };
            tokio::spawn(schedule_timeout(timeout, pong_ok, pong));
        }
        (ping, Pong { pong })
    }
}

async fn schedule_timeout(timeout: Duration, pong_ok: oneshot::Receiver<()>, pong: Pong) {
    let timeout = futures::future::select(pong_ok, tokio::time::sleep(timeout).shared()).await;
    match timeout {
        Either::Left(_pong_ok) => (),
        Either::Right(_timeout) => pong.ko(),
    }
}

impl Future for Ping {
    type Output = Result<(Self, Pong), ()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match ready!(self.ping.poll_unpin(cx)) {
            Ok(Ok(())) => {
                let (next_ping, next_pong) = Self::new(self.timeout);
                let _maybe_sent = self.ping_ok.take().map(|ping_ok| ping_ok.send(()));
                Poll::Ready(Ok((next_ping, next_pong)))
            }
            Ok(Err(())) => {
                println!("Ping returned an error");
                Poll::Ready(Err(()))
            }
            Err(RecvError { .. }) => {
                println!("Ping recv error");
                Poll::Ready(Err(()))
            }
        }
    }
}

impl Pong {
    pub fn ok(self) {
        self.pong(Ok(()));
    }

    fn ko(self) {
        self.pong(Err(()));
    }

    fn pong(self, result: Result<(), ()>) {
        if let Ok(mut pong) = self.pong.lock() {
            let _maybe_sent = pong.take().map(|pong| pong.send(result));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    #[tokio::test]
    async fn failure() {
        let (ping, _pong) = super::Ping::new(Duration::from_millis(100));
        assert!(ping.await.is_err());
    }

    #[tokio::test]
    async fn success() {
        let (ping, pong) = super::Ping::new(Duration::from_secs(1));
        tokio::spawn(async move {
            let () = tokio::time::sleep(Duration::from_millis(100)).await;
            pong.ok();
        });
        let (ping, pong) = ping.await.unwrap();
        tokio::spawn(async move {
            let () = tokio::time::sleep(Duration::from_millis(100)).await;
            pong.ok();
        });
        assert!(ping.await.is_ok());
    }
}
