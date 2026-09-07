use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use std::task::ready;
use std::time::Duration;

use futures::FutureExt as _;
use futures::future::Either;
use pin_project::pin_project;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;

use crate::consts::HEALTH_CHECK_PERIOD;
use crate::consts::HEALTH_CHECK_TIMEOUT;

/// Represents a future that resolves when it is time to send a ping.
#[pin_project(project = PingProj)]
pub enum Ping {
    Waiting {
        #[pin]
        sleep: tokio::time::Sleep,
        config: PingConfig,
    },
    Expecting {
        #[pin]
        ping: oneshot::Receiver<Result<(), ()>>,
        ping_ok: Option<oneshot::Sender<()>>,
        config: PingConfig,
    },
}

#[derive(Clone, Copy)]
pub struct PingConfig {
    pub period: Duration,
    pub timeout: Duration,
}

impl Default for PingConfig {
    fn default() -> Self {
        Self {
            period: HEALTH_CHECK_PERIOD,
            timeout: HEALTH_CHECK_TIMEOUT,
        }
    }
}

/// Represents a struct that must be called when receiving a pong
pub struct Pong {
    pong: Arc<Mutex<Option<oneshot::Sender<Result<(), ()>>>>>,
}

impl Ping {
    pub fn new(config: PingConfig) -> Self {
        Self::Waiting {
            sleep: tokio::time::sleep(config.period),
            config,
        }
    }

    fn expecting(config: PingConfig) -> (Self, Pong) {
        let (pong, ping) = oneshot::channel();
        let (ping_ok, pong_ok) = oneshot::channel();
        let ping = Self::Expecting {
            ping,
            ping_ok: Some(ping_ok),
            config,
        };
        let pong = Arc::new(Mutex::new(Some(pong)));
        {
            let pong = Pong { pong: pong.clone() };
            tokio::spawn(schedule_timeout(config.timeout, pong_ok, pong));
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
    type Output = Result<(Self, Option<Pong>), ()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            PingProj::Waiting { sleep, config } => {
                let () = ready!(sleep.poll(cx));
                let (next_ping, next_pong) = Self::expecting(*config);
                Poll::Ready(Ok((next_ping, Some(next_pong))))
            }
            PingProj::Expecting {
                ping,
                ping_ok,
                config,
            } => match ready!(ping.poll(cx)) {
                Ok(Ok(())) => {
                    let next_ping = Self::new(*config);
                    let _maybe_sent = ping_ok.take().map(|ping_ok| ping_ok.send(()));
                    Poll::Ready(Ok((next_ping, None)))
                }
                Ok(Err(())) => {
                    println!("Ping returned an error");
                    Poll::Ready(Err(()))
                }
                Err(RecvError { .. }) => {
                    println!("Ping recv error");
                    Poll::Ready(Err(()))
                }
            },
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

    use crate::ping::PingConfig;

    #[tokio::test]
    async fn failure() {
        let (ping, _pong) = super::Ping::expecting(PingConfig {
            timeout: Duration::from_millis(100),
            ..Default::default()
        });
        assert!(ping.await.is_err());
    }

    #[tokio::test]
    async fn success() {
        let (ping, pong) = super::Ping::expecting(PingConfig {
            period: Duration::from_millis(100),
            ..Default::default()
        });
        tokio::spawn(async move {
            let () = tokio::time::sleep(Duration::from_millis(100)).await;
            pong.ok();
        });

        let (ping, pong) = ping.await.unwrap();
        assert!(pong.is_none());
        let (ping, pong) = ping.await.unwrap();
        assert!(pong.is_some());
        let pong = pong.unwrap();

        tokio::spawn(async move {
            let () = tokio::time::sleep(Duration::from_millis(100)).await;
            pong.ok();
        });

        let (ping, pong) = ping.await.unwrap();
        assert!(pong.is_none());
        let (ping, pong) = ping.await.unwrap();
        assert!(pong.is_some());
        assert!(ping.await.is_err());
    }
}
