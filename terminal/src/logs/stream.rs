use server_fn::Http;
use server_fn::ServerFnError;
use server_fn::codec::Json;
use server_fn::codec::StreamingText;
use server_fn::codec::TextStream;
use terrazzo::server;

use crate::api::client_address::ClientAddress;

#[server(protocol = Http<Json, StreamingText>)]
pub async fn stream(remote: ClientAddress) -> Result<TextStream<ServerFnError>, ServerFnError> {
    imp::stream_logs(remote).await
}

#[cfg(feature = "server")]
mod imp {
    use futures::Stream;
    use futures::StreamExt as _;
    use futures::TryStreamExt as _;
    use nameth::nameth;
    use scopeguard::guard;
    use server_fn::ServerFnError;
    use server_fn::codec::TextStream;
    use tracing::info;

    use crate::api::client_address::ClientAddress;
    use crate::backend::client_service::remote_fn_service;
    use crate::logs::event::LogEvent;
    use crate::logs::state::LogState;
    use crate::utils::ndjson::serialize_line;

    #[nameth]
    pub(super) async fn stream_logs(
        remote: ClientAddress,
    ) -> Result<TextStream<ServerFnError>, ServerFnError> {
        let stream = STREAM_LOGS_FN.call(remote, ()).await?;
        let stream = stream.map_ok(|event| serialize_log_event(&event));
        Ok(TextStream::new(stream.map_err(|error| error.into())))
    }

    remote_fn_service::streaming::declare_remote_fn!(
        STREAM_LOGS_FN,
        STREAM_LOGS,
        (),
        LogEvent,
        |_server, ()| { local_logs_stream().map(Ok::<LogEvent, tonic::Status>) }
    );

    fn local_logs_stream() -> impl Stream<Item = LogEvent> {
        info!("Log stream start");
        let end = guard((), |_| info!("Log stream end"));
        let subscription = LogState::get().subscribe();
        let stream = futures::stream::unfold(subscription, |mut subscription| async move {
            let next = if let Some(event) = subscription.backlog.pop_front() {
                Some(event)
            } else {
                subscription.receiver.recv().await
            }?;
            Some(((*next).clone(), subscription))
        });
        stream.inspect(move |_log_event: &LogEvent| {
            let _ = &end;
        })
    }

    fn serialize_log_event(event: &LogEvent) -> String {
        serialize_line(event).unwrap_or_else(|error| {
            serialize_line(&LogEvent {
                id: event.id,
                level: event.level,
                message: format!("Failed to serialize log event: {error}"),
                timestamp_ms: event.timestamp_ms,
                file: None,
            })
            .expect("serialize fallback log event")
        })
    }
}

#[cfg(test)]
#[cfg(feature = "server")]
mod tests {
    use futures::StreamExt as _;
    use tracing::info;
    use tracing::warn;

    use crate::api::client_address::ClientAddress;
    use crate::logs::stream::stream;
    use crate::logs::tests::TestGuard;

    #[tokio::test]
    async fn stream_logs_replays_backlog_and_then_live_events() {
        let guard = TestGuard::get();
        guard.with_test_subscriber(|| {
            info!("backlog");
        });

        let mut stream = stream(ClientAddress::default())
            .await
            .expect("stream")
            .into_inner();
        let backlog = stream.next().await.expect("item").expect("data");
        assert!(
            backlog.contains("backlog"),
            "Expected {backlog} contains backlog"
        );

        guard.with_test_subscriber(|| {
            warn!("live");
        });

        let live = stream.next().await.expect("item").expect("data");
        assert!(live.contains("live"), "Expected {live} contains live");
    }
}
