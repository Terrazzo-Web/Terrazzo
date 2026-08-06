use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::MutexGuard;

use futures::StreamExt as _;
use futures::channel::mpsc;
use server_fn::ServerFnError;

use crate::terminal_id::TerminalId;

pub fn stream_registrations() -> MutexGuard<'static, Option<StreamRegistrations>> {
    static REGISTRATIONS: Mutex<Option<StreamRegistrations>> = Mutex::new(None);
    REGISTRATIONS.lock().expect("StreamRegistrations")
}

pub type StreamRegistrations = HashMap<TerminalId, mpsc::Sender<Result<String, ServerFnError>>>;

pub struct StreamRegistration {
    pub(super) terminal_id: TerminalId,
    pub(super) rx: mpsc::Receiver<Result<String, ServerFnError>>,
}

impl Drop for StreamRegistration {
    fn drop(&mut self) {
        let mut lock = stream_registrations();
        if let Some(stream_registrations) = &mut *lock {
            stream_registrations.remove(&self.terminal_id);
        }
    }
}

impl futures::Stream for StreamRegistration {
    type Item = Result<String, ServerFnError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.rx.poll_next_unpin(cx)
    }
}
