#![cfg(feature = "terminal")]

use std::collections::HashMap;
use std::io::ErrorKind;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::OnceLock;
use std::task::Poll;
use std::task::ready;

use futures::FutureExt as _;
use futures::Stream;
use futures::channel::oneshot;
use pin_project::pin_project;
use pin_project::pinned_drop;
use terrazzo_pty::lease::LeaseItem;
use terrazzo_pty::lease::ProcessOutputLease;
use tracing::debug;

use crate::api::shared::terminal_schema::STREAMING_WINDOW_SIZE;
use crate::terminal_id::TerminalId;

fn streams() -> MutexGuard<'static, HashMap<TerminalId, Arc<Mutex<ThrottlingState>>>> {
    static STREAMS: OnceLock<Mutex<HashMap<TerminalId, Arc<Mutex<ThrottlingState>>>>> =
        OnceLock::new();
    STREAMS.get_or_init(Mutex::default).lock().expect("streams")
}

pub fn ack(terminal_id: &TerminalId, token: u64, event: u64, end: u64) {
    let Some(throttling_state) = streams().get(terminal_id).cloned() else {
        return;
    };
    let mut throttling_state = throttling_state.lock().expect("throttling_state");
    // TODO: I don't understand why there are any changes to throttling. The PR should be more focused on periodic xtermjs snapshots without changing the throttling behavior. The focused change is that when the browser uploads a snapshot, it gets recorded and re-opening a terminal serves snapshot + data stream since the snapshot.
    if throttling_state.token != token || end > throttling_state.sent {
        return;
    }
    throttling_state.applied = throttling_state.applied.max(end);
    throttling_state.applied_event = throttling_state
        .applied_event
        .max(event.min(throttling_state.sent_event));
    if throttling_state
        .sent
        .saturating_sub(throttling_state.applied)
        >= STREAMING_WINDOW_SIZE as u64
        || throttling_state
            .sent_event
            .saturating_sub(throttling_state.applied_event)
            >= 128
    {
        return;
    }
    let Some(signal) = throttling_state.signal.take() else {
        // Applied acknowledgements can arrive before the backend is throttled.
        debug!("Missing signal to ack");
        return;
    };
    debug!("Found signal to ack");
    let _ = signal.send(());
}

#[pin_project(PinnedDrop)]
pub struct ThrottleProcessOutput {
    terminal_id: TerminalId,
    state: Arc<Mutex<ThrottlingState>>,
    #[pin]
    stream: ProcessOutputLease,
}

#[derive(Default)]
struct ThrottlingState {
    token: u64,
    sent: u64,
    applied: u64,
    sent_event: u64,
    applied_event: u64,
    signal: Option<oneshot::Sender<()>>,
    throttled: Option<oneshot::Receiver<()>>,
}

#[pinned_drop]
impl PinnedDrop for ThrottleProcessOutput {
    fn drop(self: Pin<&mut Self>) {
        let mut streams = streams();
        if streams
            .get(&self.terminal_id)
            .is_some_and(|state| Arc::ptr_eq(state, &self.state))
        {
            streams.remove(&self.terminal_id);
        }
    }
}

impl ThrottleProcessOutput {
    pub fn new(
        terminal_id: TerminalId,
        token: u64,
        initial_end: u64,
        initial_event: u64,
        stream: ProcessOutputLease,
    ) -> Self {
        let state = Arc::new(Mutex::new(ThrottlingState {
            token,
            sent: initial_end,
            applied: initial_end,
            sent_event: initial_event,
            applied_event: initial_event,
            ..Default::default()
        }));
        streams().insert(terminal_id.clone(), state.clone());
        Self {
            terminal_id,
            state,
            stream,
        }
    }
}

impl Stream for ThrottleProcessOutput {
    type Item = <ProcessOutputLease as Stream>::Item;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let mut state = this.state.lock().expect("throttling state");

        // A throttled stream waits on credit, so revocation needs its own waker.
        this.stream
            .as_ref()
            .get_ref()
            .register_stop_waker(cx.waker());
        // Lease revocation must be polled even while output credit is exhausted.
        if this.stream.as_ref().get_ref().stop_requested() {
            return this.stream.poll_next(cx);
        }

        // Check if the stream is in throttled state.
        if let Some(mut throttled) = state.throttled.take() {
            match throttled.poll_unpin(cx) {
                Poll::Ready(Ok(())) => {}
                Poll::Ready(Err(oneshot::Canceled)) => {
                    return Poll::Ready(Some(LeaseItem::Error(std::io::Error::new(
                        ErrorKind::BrokenPipe,
                        "Process output throtting signal got canceled",
                    ))));
                }
                Poll::Pending => {
                    // Continue waiting on the throttle state.
                    state.throttled = Some(throttled);
                    return Poll::Pending;
                }
            }
        }

        let result = ready!(this.stream.poll_next(cx));
        if let Some(LeaseItem::Data(record)) = &result {
            state.sent_event = record.id;
            match &record.event {
                terrazzo_pty::replay::Event::Output { end, .. } => state.sent = *end,
                terrazzo_pty::replay::Event::Gap { end, .. } => {
                    state.sent = *end;
                    state.applied = *end;
                    state.applied_event = record.id;
                }
                _ => (),
            }

            // Enter throttled state if too many bytes were sent since last ack.
            if state.sent.saturating_sub(state.applied) >= STREAMING_WINDOW_SIZE as u64
                || state.sent_event.saturating_sub(state.applied_event) >= 128
            {
                let (signal, throttled) = oneshot::channel();
                state.signal = Some(signal);
                state.throttled = Some(throttled);
            }
        }
        return Poll::Ready(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_throttle_drop_preserves_new_attachment() {
        let id: TerminalId = "snapshot-throttle-generation-test".into();
        let old = ThrottleProcessOutput::new(id.clone(), 1, 0, 0, ProcessOutputLease::Revoked);
        let new = ThrottleProcessOutput::new(id.clone(), 2, 0, 0, ProcessOutputLease::Revoked);
        drop(old);
        assert!(
            streams()
                .get(&id)
                .is_some_and(|state| Arc::ptr_eq(state, &new.state))
        );
        drop(new);
        assert!(!streams().contains_key(&id));
    }

    #[test]
    fn ack_applied_is_monotonic_and_attachment_scoped() {
        let id: TerminalId = "snapshot-throttle-ack-test".into();
        let stream = ThrottleProcessOutput::new(id.clone(), 2, 0, 0, ProcessOutputLease::Revoked);
        stream.state.lock().unwrap().sent = 100;
        ack(&id, 1, 0, 100);
        ack(&id, 2, 0, 101);
        assert_eq!(stream.state.lock().unwrap().applied, 0);
        ack(&id, 2, 0, 50);
        ack(&id, 2, 0, 40);
        assert_eq!(stream.state.lock().unwrap().applied, 50);
    }
}
