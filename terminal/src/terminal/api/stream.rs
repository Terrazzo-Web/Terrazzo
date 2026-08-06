use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use futures::StreamExt;
use futures::channel::mpsc;
use server_fn::Http;
use server_fn::ServerFnError;
use server_fn::codec::Json;
use server_fn::codec::StreamingText;
use server_fn::codec::TextStream;
use terrazzo::server;
use tonic::transport::Server;

use crate::api::shared::terminal_schema::RegisterTerminalMode;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::terminal_id::TerminalId;

#[server(protocol = Http<Json, StreamingText>)]
pub async fn stream(
    mode: RegisterTerminalMode,
    terminal_def: TerminalDef,
) -> Result<TextStream, ServerFnError> {
    crate::terminal::service::stream::stream(mode, terminal_def).await
}

type StreamRegistrations = HashMap<TerminalId, Arc<StreamRegistration>>;

fn stream_registrations() -> MutexGuard<'static, Option<StreamRegistrations>> {
    static REGISTRATIONS: Mutex<Option<StreamRegistrations>> = Mutex::new(None);
    REGISTRATIONS.lock().expect("StreamRegistrations")
}

struct StreamRegistration {
    terminal_id: TerminalId,
    rx: mpsc::Receiver<Result<String, ServerFnError>>,
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
