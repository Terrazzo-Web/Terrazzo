use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;
use std::task::ready;

use bytes::Bytes;
use futures::Stream;
use futures::StreamExt as _;
use futures::channel::oneshot;
use futures::lock::Mutex;
use futures::stream::TakeUntil;
use nameth::NamedEnumValues as _;
use nameth::NamedType as _;
use nameth::nameth;
use scopeguard::defer;
use tracing::debug;
use tracing::debug_span;
use tracing::info;
use tracing::trace;

use crate::ProcessIO;
use crate::ProcessInput;
use crate::ProcessOutput;
use crate::release_on_drop::ReleaseOnDrop;

#[nameth]
pub struct ProcessIoEntry {
    input: Mutex<ProcessInput>,
    output: Mutex<Option<ProcessOutputExchange>>,
}

impl ProcessIoEntry {
    pub fn new(process_io: ProcessIO) -> Arc<Self> {
        info!("Create {}", Self::type_name());
        let (input, output) = process_io.split();
        Arc::new(Self {
            input: Mutex::new(input),
            output: Mutex::new(Some(ProcessOutputExchange::new(output))),
        })
    }

    pub async fn lease_output(
        self: &Arc<Self>,
        rewind: bool,
    ) -> Result<ProcessOutputLease, LeaseProcessOutputError> {
        let mut lock = self.output.lock().await;
        let exchange = lock.as_mut().ok_or(LeaseProcessOutputError::OutputNotSet)?;
        return Ok(exchange.lease(rewind).await?);
    }

    pub async fn input(&self) -> futures::lock::MutexGuard<'_, ProcessInput> {
        self.input.lock().await
    }
}

impl Drop for ProcessIoEntry {
    fn drop(&mut self) {
        info!("Drop {}", Self::type_name());
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum LeaseProcessOutputError {
    #[error("[{n}] Output not set", n = self.name())]
    OutputNotSet,

    #[error("[{n}] {0}", n = self.name())]
    LeaseError(#[from] LeaseError),
}

struct ProcessOutputExchange {
    signal_tx: Option<oneshot::Sender<()>>,
    process_output_rx: oneshot::Receiver<ProcessOutput>,
}

impl ProcessOutputExchange {
    fn new(process_output: ProcessOutput) -> Self {
        let (_lease, signal_tx, process_output_rx) = ProcessOutputLease::new(process_output);
        Self {
            signal_tx: Some(signal_tx),
            process_output_rx,
        }
    }

    async fn lease(&mut self, rewind: bool) -> Result<ProcessOutputLease, LeaseError> {
        if let Some(signal_tx) = self.signal_tx.take() {
            match signal_tx.send(()) {
                Ok(()) => debug!("Current lease was stopped"),
                Err(()) => debug!("The process was not leased"),
            }
        }
        debug!("Getting new lease...");
        // Keep the receiver in the entry so a canceled handoff can be resumed.
        let mut process_output = (&mut self.process_output_rx).await?;
        if rewind {
            process_output.0.rewind();
        }
        debug!("Getting new lease: Done");
        let (lease, signal_tx, process_output_rx) = ProcessOutputLease::new(process_output);
        *self = Self {
            signal_tx: Some(signal_tx),
            process_output_rx,
        };
        Ok(lease)
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum LeaseError {
    #[error("[{n}] Canceled", n = self.name())]
    Canceled(#[from] oneshot::Canceled),
}

#[nameth]
pub enum ProcessOutputLease {
    /// The process is active and this is the current lease.
    Leased(TakeUntil<ReleaseOnDrop<ProcessOutput>, oneshot::Receiver<()>>),

    /// The process is still active but another client is consuming the stream.
    Revoked,

    /// The process is closed. We return one last [LeaseItem] to indicate the closure.
    Closed,
}

impl ProcessOutputLease {
    fn new(
        process_output: ProcessOutput,
    ) -> (Self, oneshot::Sender<()>, oneshot::Receiver<ProcessOutput>) {
        let (process_output, process_output_rx) = ReleaseOnDrop::new(process_output);
        let (signal_tx, signal_rx) = oneshot::channel();
        let process_output = process_output.take_until(signal_rx);
        let lease = Self::Leased(process_output);
        (lease, signal_tx, process_output_rx)
    }

    fn revoke(&mut self) {
        let _span = debug_span!("Revoking").entered();
        debug!("Start");
        defer!(debug!("End"));
        *self = Self::Revoked
    }
}

impl Stream for ProcessOutputLease {
    type Item = LeaseItem;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        trace!("Poll next: state={}", self.name());
        let next = {
            let process_io = match &mut *self {
                ProcessOutputLease::Leased(process_io) => process_io,
                ProcessOutputLease::Revoked => return None.into(),
                ProcessOutputLease::Closed => {
                    self.revoke();
                    return Some(LeaseItem::EOS).into();
                }
            };
            let next = ready!(process_io.poll_next_unpin(cx));
            if next.is_none() && process_io.is_stopped() {
                match process_io.take_result() {
                    Some(Err(oneshot::Canceled)) | None => {
                        debug!("The process ended");
                        self.revoke();
                        return Some(LeaseItem::EOS).into();
                    }
                    Some(Ok(())) => debug!("The lease was revoked"),
                }
            }
            trace! { "next.is_none={} process_io.is_stopped={}", next.is_none(), process_io.is_stopped() };
            next
        };

        Some(match next {
            Some(Ok(data)) => {
                debug_assert!(!data.is_empty(), "Unexpected empty buffer");
                trace! { "Reading {}", String::from_utf8_lossy(&data).escape_default() }
                LeaseItem::Data(data)
            }
            Some(Err(error)) => {
                trace!("Reading failed: {error}");
                LeaseItem::Error(error)
            }
            None => {
                debug!("next is None");
                self.revoke();
                return None.into();
            }
        })
        .into()
    }
}

#[nameth]
#[derive(Debug)]
pub enum LeaseItem {
    EOS,
    Data(Bytes),
    Error(std::io::Error),
}

impl Stream for ReleaseOnDrop<ProcessOutput> {
    type Item = <ProcessOutput as Stream>::Item;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        self.get_mut().as_mut().poll_next_unpin(cx)
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt as _;
    use futures::stream;

    use super::*;
    use crate::pty::Pty;
    use crate::tail::TailStream;

    fn entry() -> Arc<ProcessIoEntry> {
        let (_, input) = Pty::new().unwrap().into_split();
        let output = TailStream::new(
            stream::once(async { Ok(Bytes::from_static(b"output")) }).chain(stream::pending()),
            1024,
        );
        Arc::new(ProcessIoEntry {
            input: Mutex::new(ProcessInput(input)),
            output: Mutex::new(Some(ProcessOutputExchange::new(ProcessOutput(output)))),
        })
    }

    #[tokio::test]
    async fn canceled_handoff_can_be_resumed() {
        let entry = entry();
        let mut original = entry.lease_output(false).await.unwrap();
        assert!(matches!(original.next().await, Some(LeaseItem::Data(_))));

        // Simulate a reconnect being canceled while the old stream is not polled.
        let mut handoff = Box::pin(entry.lease_output(false));
        assert!(futures::poll!(&mut handoff).is_pending());
        drop(handoff);

        // Repeated canceled attempts must preserve the pending output receiver.
        let mut handoff = Box::pin(entry.lease_output(false));
        assert!(futures::poll!(&mut handoff).is_pending());
        drop(handoff);
        assert!(original.next().await.is_none());

        let mut replacement = entry.lease_output(true).await.unwrap();
        match replacement.next().await {
            Some(LeaseItem::Data(data)) => assert_eq!(data, Bytes::from_static(b"output")),
            item => panic!("Expected the original process output, got {item:?}"),
        }
    }

    #[tokio::test]
    async fn canceled_handoff_preserves_output_returned_by_drop() {
        let entry = entry();
        let original = entry.lease_output(false).await.unwrap();
        let mut handoff = Box::pin(entry.lease_output(false));
        assert!(futures::poll!(&mut handoff).is_pending());
        drop(original);
        // The output is ready in the receiver, but this request never polls again.
        drop(handoff);

        let mut replacement = entry.lease_output(false).await.unwrap();
        assert!(matches!(replacement.next().await, Some(LeaseItem::Data(_))));
    }

    #[tokio::test]
    async fn concurrent_handoffs_are_serialized() {
        let entry = entry();
        let mut original = entry.lease_output(false).await.unwrap();
        let mut first = Box::pin(entry.lease_output(false));
        let mut second = Box::pin(entry.lease_output(false));
        assert!(futures::poll!(&mut first).is_pending());
        assert!(futures::poll!(&mut second).is_pending());
        assert!(original.next().await.is_none());

        let mut first = first.await.unwrap();
        assert!(futures::poll!(&mut second).is_pending());
        assert!(first.next().await.is_none());
        let mut second = second.await.unwrap();
        assert!(matches!(second.next().await, Some(LeaseItem::Data(_))));
    }
}
