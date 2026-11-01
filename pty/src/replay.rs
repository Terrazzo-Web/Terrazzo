//! Bounded PTY replay. The reader keeps running when consumers disconnect.
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

use bytes::Bytes;
use futures::Stream;
use futures::StreamExt as _;

#[derive(Clone, Debug)]
pub enum Event {
    Output { end: u64, bytes: Bytes },
    Resize { rows: u16, cols: u16 },
    Gap { end: u64, rows: u16, cols: u16 },
}

#[derive(Clone, Debug)]
pub struct Record {
    pub id: u64,
    pub event: Event,
}

#[derive(Clone, Debug)]
pub struct Checkpoint {
    pub event: u64,
    pub end: u64,
    pub rows: u16,
    pub cols: u16,
    pub payload: Bytes,
}

pub struct State {
    records: VecDeque<Record>,
    bytes: usize,
    capacity: usize,
    next: u64,
    end: u64,
    base: u64,
    base_end: u64,
    base_size: (u16, u16),
    size: (u16, u16),
    closed: bool,
    error: Option<String>,
    waker: Option<Waker>,
    pub checkpoint: Option<Checkpoint>,
    pub attachment: u64,
    pub sent: u64,
    pub applied: u64,
    points: VecDeque<(u64, u64, (u16, u16))>,
    applied_point: (u64, u64, (u16, u16)),
}

#[derive(Clone)]
pub struct Journal(pub Arc<Mutex<State>>);

impl Journal {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self(Arc::new(Mutex::new(State {
            records: VecDeque::new(),
            bytes: 0,
            capacity,
            next: 1,
            end: 0,
            base: 0,
            base_end: 0,
            base_size: (24, 80),
            size: (24, 80),
            closed: false,
            error: None,
            waker: None,
            checkpoint: None,
            attachment: 0,
            sent: 0,
            applied: 0,
            points: VecDeque::new(),
            applied_point: (0, 0, (24, 80)),
        })))
    }

    pub fn output(&self, mut bytes: Bytes) {
        if bytes.is_empty() {
            return;
        }
        let mut state = self.0.lock().unwrap();
        state.end += bytes.len() as u64;
        // Copy an oversized suffix so its allocation does not retain the discarded prefix.
        if bytes.len() > state.capacity {
            bytes = Bytes::copy_from_slice(&bytes[bytes.len() - state.capacity..]);
            while !state.records.is_empty() {
                state.evict();
            }
            state.base = state.next;
            state.next += 1;
            state.base_end = state.end - bytes.len() as u64;
        }
        let end = state.end;
        state.push(Event::Output { end, bytes });
    }

    pub fn resize_with<E>(
        &self,
        token: u64,
        rows: u16,
        cols: u16,
        apply: impl FnOnce() -> Result<(), E>,
    ) -> Result<bool, E> {
        let mut state = self.0.lock().unwrap();
        if state.attachment != token {
            return Ok(false);
        }
        apply()?;
        if state.size != (rows, cols) {
            state.size = (rows, cols);
            state.push(Event::Resize { rows, cols });
        }
        Ok(true)
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        let mut state = self.0.lock().unwrap();
        if state.size == (rows, cols) {
            return;
        }
        state.size = (rows, cols);
        state.push(Event::Resize { rows, cols });
    }

    pub fn close(&self, error: Option<String>) {
        let mut state = self.0.lock().unwrap();
        state.closed = true;
        state.error = error;
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }

    pub fn reader(&self, cursor: u64) -> Reader {
        Reader {
            journal: self.clone(),
            cursor,
            done: false,
        }
    }

    pub fn attach(
        &self,
        resume: Option<u64>,
    ) -> Result<(u64, Option<Checkpoint>, u64), &'static str> {
        let mut state = self.0.lock().unwrap();
        if resume.is_some_and(|cursor| cursor >= state.next) {
            return Err("InvalidCursor");
        }
        let checkpoint = if resume.is_none() {
            state.checkpoint.clone()
        } else {
            None
        };
        let cursor = resume.unwrap_or_else(|| checkpoint.as_ref().map_or(0, |c| c.event));
        let point = if let Some(cursor) = resume {
            state
                .points
                .iter()
                .find(|point| point.0 == cursor)
                .copied()
                .or_else(|| (state.applied_point.0 == cursor).then_some(state.applied_point))
                .ok_or("InvalidCursor")?
        } else {
            checkpoint
                .as_ref()
                .map_or((0, 0, (24, 80)), |c| (c.event, c.end, (c.rows, c.cols)))
        };
        state.attachment += 1;
        state.points.clear();
        state.points.push_back(point);
        state.applied_point = point;
        state.sent = cursor;
        state.applied = cursor;
        Ok((state.attachment, checkpoint, cursor))
    }

    pub fn validate(&self, token: u64) -> Result<(), &'static str> {
        if self.0.lock().unwrap().attachment == token {
            Ok(())
        } else {
            Err("StaleAttachment")
        }
    }

    pub fn mark_sent(&self, token: u64, record: &Record) -> bool {
        let mut state = self.0.lock().unwrap();
        if state.attachment != token {
            return false;
        }
        let (_, mut end, mut size) = state.points.back().copied().unwrap_or((0, 0, (24, 80)));
        match &record.event {
            Event::Output { end: next, .. } => end = *next,
            Event::Resize { rows, cols } => size = (*rows, *cols),
            Event::Gap {
                end: next,
                rows,
                cols,
            } => {
                end = *next;
                size = (*rows, *cols);
            }
        }
        state.sent = state.sent.max(record.id);
        state.points.push_back((record.id, end, size));
        while state.points.len() > state.capacity.div_ceil(128).clamp(256, 4096) {
            state.points.pop_front();
        }
        true
    }

    pub fn validate_end(&self, token: u64, cursor: u64, end: u64) -> Result<(), &'static str> {
        let state = self.0.lock().unwrap();
        if state.attachment != token {
            return Err("StaleAttachment");
        }
        if !(state.applied_point.0 == cursor && state.applied_point.1 == end)
            && !state.points.iter().any(|p| p.0 == cursor && p.1 == end)
        {
            return Err("InvalidCursor");
        }
        Ok(())
    }

    pub fn ack(&self, token: u64, cursor: u64) -> Result<(), &'static str> {
        let mut state = self.0.lock().unwrap();
        if state.attachment != token {
            return Err("StaleAttachment");
        }
        if cursor > state.sent {
            return Err("InvalidCursor");
        }
        if cursor > state.applied {
            state.applied_point = state
                .points
                .iter()
                .find(|p| p.0 == cursor)
                .copied()
                .ok_or("InvalidCursor")?;
            state.applied = cursor;
        }
        Ok(())
    }

    pub fn commit(&self, token: u64, checkpoint: Checkpoint) -> Result<bool, &'static str> {
        let mut state = self.0.lock().unwrap();
        if state.attachment != token {
            return Err("StaleAttachment");
        }
        if checkpoint.payload.len() > state.capacity {
            return Err("TooLarge");
        }
        if let Some(old) = &state.checkpoint {
            if checkpoint.event < old.event {
                return Err("InvalidCursor");
            }
            if checkpoint.event == old.event {
                return if checkpoint.end == old.end
                    && checkpoint.rows == old.rows
                    && checkpoint.cols == old.cols
                    && checkpoint.payload == old.payload
                {
                    Ok(false)
                } else {
                    Err("InvalidCursor")
                };
            }
        }
        if !state.points.iter().any(|p| {
            p.0 == checkpoint.event
                && p.1 == checkpoint.end
                && p.2 == (checkpoint.rows, checkpoint.cols)
        }) {
            return Err("InvalidCursor");
        }
        if checkpoint.rows == 0
            || checkpoint.cols == 0
            || checkpoint.event > state.applied
            || checkpoint.end > state.end
        {
            return Err("InvalidCursor");
        }
        while state
            .records
            .front()
            .is_some_and(|record| record.id <= checkpoint.event)
        {
            state.evict();
        }
        state.checkpoint = Some(checkpoint);
        Ok(true)
    }

    pub fn high_water(&self) -> u64 {
        self.0.lock().unwrap().next - 1
    }
}

impl State {
    fn evict(&mut self) {
        if let Some(record) = self.records.pop_front() {
            self.base = record.id;
            match record.event {
                Event::Output { end, bytes } => {
                    self.bytes -= bytes.len();
                    self.base_end = end;
                }
                Event::Resize { rows, cols } => self.base_size = (rows, cols),
                Event::Gap { .. } => unreachable!(),
            }
        }
    }

    fn push(&mut self, event: Event) {
        if let Event::Output { bytes, .. } = &event {
            self.bytes += bytes.len();
        }
        let id = self.next;
        self.next += 1;
        self.records.push_back(Record { id, event });
        // Bound metadata even when there is no output.
        while self.bytes > self.capacity
            || self.records.len() > self.capacity.div_ceil(128).clamp(32, 4096)
        {
            self.evict();
        }
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}

pub struct Reader {
    journal: Journal,
    pub cursor: u64,
    done: bool,
}

impl Stream for Reader {
    type Item = std::io::Result<Record>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.done {
            return Poll::Ready(None);
        }
        let journal = self.journal.clone();
        let mut state = journal.0.lock().unwrap();
        if self.cursor < state.base {
            self.cursor = state.base;
            return Poll::Ready(Some(Ok(Record {
                id: state.base,
                event: Event::Gap {
                    end: state.base_end,
                    rows: state.base_size.0,
                    cols: state.base_size.1,
                },
            })));
        }
        if let Some(record) = state.records.iter().find(|r| r.id > self.cursor).cloned() {
            self.cursor = record.id;
            return Poll::Ready(Some(Ok(record)));
        }
        if state.closed {
            self.done = true;
            return Poll::Ready(
                state
                    .error
                    .as_ref()
                    .map(|error| Err(std::io::Error::other(error.clone()))),
            );
        }
        state.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

/// Owns the output worker. Dropping the last handle stops it.
pub struct Worker(tokio::task::JoinHandle<()>);
impl Drop for Worker {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub fn spawn<S>(stream: S, capacity: usize) -> (Journal, Arc<Worker>)
where
    S: Stream<Item = std::io::Result<Bytes>> + Send + 'static,
{
    let journal = Journal::new(capacity);
    let worker = journal.clone();
    let handle = tokio::spawn(async move {
        futures::pin_mut!(stream);
        while let Some(item) = stream.next().await {
            match item {
                Ok(bytes) => worker.output(bytes),
                Err(error) => {
                    worker.close(Some(error.to_string()));
                    return;
                }
            }
        }
        worker.close(None);
    });
    (journal, Arc::new(Worker(handle)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn capacity_overflow_evicts_oldest_without_stopping_reader() {
        let journal = Journal::new(6);
        for _ in 0..10 {
            journal.output(Bytes::from_static(b"abc"));
        }
        let mut reader = journal.reader(0);
        assert!(matches!(
            reader.next().await.unwrap().unwrap().event,
            Event::Gap { end: 24, .. }
        ));
        assert_eq!(journal.0.lock().unwrap().bytes, 6);
        assert_eq!(reader.next().await.unwrap().unwrap().id, 9);
    }
    #[tokio::test]
    async fn oversized_append_retains_bounded_suffix() {
        let journal = Journal::new(3);
        journal.output(Bytes::from_static(b"abcdef"));
        let mut reader = journal.reader(0);
        assert!(matches!(
            reader.next().await.unwrap().unwrap().event,
            Event::Gap { end: 3, .. }
        ));
        match reader.next().await.unwrap().unwrap().event {
            Event::Output { end, bytes } => {
                assert_eq!(end, 6);
                assert_eq!(bytes, b"def"[..]);
            }
            _ => panic!("expected output"),
        }
    }
    #[test]
    fn checkpoint_retry_is_idempotent() {
        let journal = Journal::new(64);
        journal.output(Bytes::from_static(b"abc"));
        let (token, _, _) = journal.attach(None).unwrap();
        journal.mark_sent(
            token,
            &Record {
                id: 1,
                event: Event::Output {
                    end: 3,
                    bytes: Bytes::from_static(b"abc"),
                },
            },
        );
        journal.ack(token, 1).unwrap();
        let checkpoint = Checkpoint {
            event: 1,
            end: 3,
            rows: 24,
            cols: 80,
            payload: Bytes::from_static(b"state"),
        };
        assert_eq!(journal.commit(token, checkpoint.clone()), Ok(true));
        assert_eq!(journal.commit(token, checkpoint), Ok(false));
        let (_, restored, cursor) = journal.attach(None).unwrap();
        assert_eq!(cursor, 1);
        assert!(restored.is_some());
        assert_eq!(journal.ack(token, 1), Err("StaleAttachment"));
    }
    #[test]
    fn resize_only_history_has_bounded_metadata() {
        let journal = Journal::new(3);
        for i in 1..10000 {
            journal.resize(24, i);
        }
        assert!(journal.0.lock().unwrap().records.len() <= 4096);
    }
    fn checkpoint(record: &Record) -> Checkpoint {
        let Event::Output { end, .. } = record.event else {
            panic!("output expected")
        };
        Checkpoint {
            event: record.id,
            end,
            rows: 24,
            cols: 80,
            payload: Bytes::from_static(b"state"),
        }
    }

    #[tokio::test]
    async fn fresh_attach_replays_checkpoint_and_exact_suffix() {
        let journal = Journal::new(64);
        let (token, _, _) = journal.attach(None).unwrap();
        journal.output(Bytes::from_static(b"first"));
        let mut reader = journal.reader(0);
        let first = reader.next().await.unwrap().unwrap();
        journal.mark_sent(token, &first);
        journal.ack(token, first.id).unwrap();
        journal.commit(token, checkpoint(&first)).unwrap();
        journal.output(Bytes::from_static(b"second"));
        let (_, restored, cursor) = journal.attach(None).unwrap();
        assert_eq!(restored.unwrap().event, first.id);
        let mut reader = journal.reader(cursor);
        assert_eq!(reader.next().await.unwrap().unwrap().id, 2);
        journal.output(Bytes::from_static(b"live"));
        assert_eq!(reader.next().await.unwrap().unwrap().id, 3);
    }

    #[tokio::test]
    async fn eviction_overtakes_reader_emits_gap() {
        let journal = Journal::new(3);
        let mut reader = journal.reader(0);
        journal.output(Bytes::from_static(b"abc"));
        assert_eq!(reader.next().await.unwrap().unwrap().id, 1);
        for _ in 0..3 {
            journal.output(Bytes::from_static(b"def"));
        }
        assert!(matches!(
            reader.next().await.unwrap().unwrap().event,
            Event::Gap { end: 9, .. }
        ));
        assert_eq!(reader.next().await.unwrap().unwrap().id, 4);
        for _ in 0..3 {
            journal.output(Bytes::from_static(b"ghi"));
        }
        assert!(matches!(
            reader.next().await.unwrap().unwrap().event,
            Event::Gap { end: 18, .. }
        ));
    }

    #[tokio::test]
    async fn checkpoint_commit_does_not_require_retained_suffix() {
        let journal = Journal::new(5);
        let (token, _, _) = journal.attach(None).unwrap();
        journal.output(Bytes::from_static(b"a"));
        let first = journal.reader(0).next().await.unwrap().unwrap();
        journal.mark_sent(token, &first);
        journal.ack(token, first.id).unwrap();
        for _ in 0..4 {
            journal.output(Bytes::from_static(b"bb"));
        }
        journal.commit(token, checkpoint(&first)).unwrap();
        let (_, _, cursor) = journal.attach(None).unwrap();
        assert!(matches!(
            journal.reader(cursor).next().await.unwrap().unwrap().event,
            Event::Gap { .. }
        ));
    }

    #[tokio::test]
    async fn snapshot_size_limit_preserves_previous_checkpoint() {
        let journal = Journal::new(5);
        let (token, _, _) = journal.attach(None).unwrap();
        journal.output(Bytes::from_static(b"a"));
        let first = journal.reader(0).next().await.unwrap().unwrap();
        journal.mark_sent(token, &first);
        journal.ack(token, first.id).unwrap();
        journal.commit(token, checkpoint(&first)).unwrap();
        let mut oversized = checkpoint(&first);
        oversized.payload = Bytes::from_static(b"too large");
        assert_eq!(journal.commit(token, oversized), Err("TooLarge"));
        assert_eq!(
            journal
                .0
                .lock()
                .unwrap()
                .checkpoint
                .as_ref()
                .unwrap()
                .payload,
            b"state"[..]
        );
    }

    #[tokio::test]
    async fn checkpoint_validation_rejects_invalid_metadata() {
        let journal = Journal::new(64);
        let (token, _, _) = journal.attach(None).unwrap();
        journal.output(Bytes::from_static(b"abc"));
        let record = journal.reader(0).next().await.unwrap().unwrap();
        assert_eq!(
            journal.commit(token, checkpoint(&record)),
            Err("InvalidCursor")
        );
        journal.mark_sent(token, &record);
        assert_eq!(
            journal.commit(token, checkpoint(&record)),
            Err("InvalidCursor")
        );
        journal.ack(token, record.id).unwrap();
        let mut invalid = checkpoint(&record);
        invalid.end = 1;
        assert_eq!(journal.commit(token, invalid), Err("InvalidCursor"));
        let mut invalid = checkpoint(&record);
        invalid.cols = 1;
        assert_eq!(journal.commit(token, invalid), Err("InvalidCursor"));
        journal.attach(None).unwrap();
        assert_eq!(
            journal.commit(token, checkpoint(&record)),
            Err("StaleAttachment")
        );
    }

    #[tokio::test]
    async fn resume_after_apply_before_ack_does_not_duplicate_output() {
        let journal = Journal::new(64);
        let (token, _, _) = journal.attach(None).unwrap();
        journal.output(Bytes::from_static(b"abc"));
        let record = journal.reader(0).next().await.unwrap().unwrap();
        journal.mark_sent(token, &record);
        journal.validate_end(token, 1, 3).unwrap();
        let (_, snapshot, cursor) = journal.attach(Some(1)).unwrap();
        assert!(snapshot.is_none());
        journal.output(Bytes::from_static(b"def"));
        assert_eq!(journal.reader(cursor).next().await.unwrap().unwrap().id, 2);
    }

    #[test]
    fn failed_resize_does_not_emit_event() {
        let journal = Journal::new(64);
        let (token, _, _) = journal.attach(None).unwrap();
        assert_eq!(
            journal.resize_with(token, 30, 100, || Err("ioctl failed")),
            Err("ioctl failed")
        );
        assert_eq!(journal.high_water(), 0);
    }

    #[test]
    fn replay_storage_stays_bounded_across_many_terminals() {
        trz_gateway_common::tracing::test_utils::enable_tracing_for_tests();
        let journals: Vec<_> = (0..32).map(|_| Journal::new(200_000)).collect();
        for journal in &journals {
            for _ in 0..200 {
                journal.output(Bytes::from(vec![b'x'; 8192]));
            }
            let state = journal.0.lock().unwrap();
            assert!(state.bytes <= 200_000);
            assert!(state.records.len() <= 4096);
        }
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        assert_eq!(unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) }, 0);
        eprintln!(
            "32 flooded terminal journals; getrusage maxrss={}",
            usage.ru_maxrss
        );
    }
    #[tokio::test]
    async fn resume_after_send_before_apply_replays_unapplied_output() {
        let journal = Journal::new(64);
        let (token, _, _) = journal.attach(None).unwrap();
        journal.output(Bytes::from_static(b"abc"));
        let record = journal.reader(0).next().await.unwrap().unwrap();
        journal.mark_sent(token, &record);
        let (_, _, cursor) = journal.attach(Some(0)).unwrap();
        assert_eq!(journal.reader(cursor).next().await.unwrap().unwrap().id, 1);
    }

    #[tokio::test]
    async fn resume_with_evicted_cursor_reports_gap() {
        let journal = Journal::new(3);
        journal.attach(None).unwrap();
        journal.output(Bytes::from_static(b"abc"));
        journal.output(Bytes::from_static(b"def"));
        let (_, _, cursor) = journal.attach(Some(0)).unwrap();
        let mut reader = journal.reader(cursor);
        assert!(matches!(
            reader.next().await.unwrap().unwrap().event,
            Event::Gap { end: 3, .. }
        ));
        assert_eq!(reader.next().await.unwrap().unwrap().id, 2);
    }

    #[tokio::test]
    async fn revoked_attachment_cannot_mutate_process() {
        let journal = Journal::new(64);
        let (old, _, _) = journal.attach(None).unwrap();
        journal.output(Bytes::from_static(b"abc"));
        let record = journal.reader(0).next().await.unwrap().unwrap();
        journal.mark_sent(old, &record);
        let (new, _, _) = journal.attach(None).unwrap();
        assert_eq!(journal.validate(old), Err("StaleAttachment"));
        assert_eq!(journal.ack(old, 1), Err("StaleAttachment"));
        assert_eq!(
            journal.commit(old, checkpoint(&record)),
            Err("StaleAttachment")
        );
        let mut applied = false;
        assert_eq!(
            journal.resize_with(old, 30, 100, || {
                applied = true;
                Ok::<_, ()>(())
            }),
            Ok(false)
        );
        assert!(!applied);
        assert!(journal.validate(new).is_ok());
    }

    #[tokio::test]
    async fn checkpoint_commit_and_append_preserve_event_order() {
        for _ in 0..32 {
            let journal = Journal::new(64);
            let (token, _, _) = journal.attach(None).unwrap();
            journal.output(Bytes::from_static(b"first"));
            let record = journal.reader(0).next().await.unwrap().unwrap();
            journal.mark_sent(token, &record);
            journal.ack(token, record.id).unwrap();
            let barrier = Arc::new(std::sync::Barrier::new(2));
            std::thread::scope(|scope| {
                let writer = journal.clone();
                let ready = barrier.clone();
                scope.spawn(move || {
                    ready.wait();
                    writer.output(Bytes::from_static(b"second"));
                });
                barrier.wait();
                journal.commit(token, checkpoint(&record)).unwrap();
            });
            let (_, restored, cursor) = journal.attach(None).unwrap();
            assert_eq!(restored.unwrap().event, 1);
            assert_eq!(journal.reader(cursor).next().await.unwrap().unwrap().id, 2);
        }
    }
    #[tokio::test]
    async fn detached_worker_keeps_reading_after_capacity_is_exceeded() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (journal, _worker) = spawn(tokio_stream::wrappers::UnboundedReceiverStream::new(rx), 3);
        for _ in 0..100 {
            tx.send(Ok(Bytes::from_static(b"abc"))).unwrap();
        }
        drop(tx);
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while !journal.0.lock().unwrap().closed {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(journal.0.lock().unwrap().bytes, 3);
        assert_eq!(journal.0.lock().unwrap().end, 300);
    }
    #[test]
    fn resume_retains_last_acknowledged_cursor_after_sent_history_eviction() {
        let journal = Journal::new(64);
        let (token, _, _) = journal.attach(None).unwrap();
        for id in 1..1000 {
            journal.output(Bytes::from_static(b"x"));
            journal.mark_sent(
                token,
                &Record {
                    id,
                    event: Event::Output {
                        end: id,
                        bytes: Bytes::from_static(b"x"),
                    },
                },
            );
        }
        journal.validate_end(token, 0, 0).unwrap();
        assert!(journal.attach(Some(0)).is_ok());
    }
}
