use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::MutexGuard;

use futures::StreamExt as _;
use futures::channel::mpsc;
use futures::channel::oneshot;
use futures::future::Shared;
use server_fn::ServerFnError;

use crate::terminal_id::TerminalId;

pub struct StreamRegistrations {
    pub map: HashMap<TerminalId, mpsc::UnboundedSender<Result<String, ServerFnError>>>,
    pub ready: Shared<oneshot::Receiver<()>>,
}

pub fn stream_registrations() -> MutexGuard<'static, Option<StreamRegistrations>> {
    static REGISTRATIONS: Mutex<Option<StreamRegistrations>> = Mutex::new(None);
    REGISTRATIONS.lock().expect("StreamRegistrations")
}

pub struct StreamRegistration {
    pub(super) terminal_id: TerminalId,
    pub(super) rx: mpsc::UnboundedReceiver<Result<String, ServerFnError>>,
}

impl Drop for StreamRegistration {
    fn drop(&mut self) {
        let mut lock = stream_registrations();
        if let Some(stream_registrations) = &mut *lock {
            stream_registrations.map.remove(&self.terminal_id);
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
