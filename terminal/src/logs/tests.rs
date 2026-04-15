#![cfg(test)]
#![cfg(feature = "server")]

use std::sync::Mutex;

use futures::FutureExt as _;
use tokio::time::Duration;
use tracing::debug;
use tracing::dispatcher::Dispatch;
use tracing::error;
use tracing::info;
use tracing::info_span;
use tracing::warn;
use tracing_subscriber::Registry;
use tracing_subscriber::layer::SubscriberExt as _;

use super::tracing::LogStreamLayer;
use crate::logs::state::LogState;

pub struct TestGuard<'t>(#[allow(dead_code)] std::sync::MutexGuard<'t, ()>);

impl TestGuard<'_> {
    pub fn get() -> Self {
        static TEST_LOCK: Mutex<()> = Mutex::new(());
        let lock = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        LogState::get().reset_for_tests();
        Self(lock)
    }

    pub fn with_test_subscriber(&self, f: impl FnOnce()) {
        let subscriber = Registry::default().with(LogStreamLayer);
        tracing::dispatcher::with_default(&Dispatch::new(subscriber), f);
    }
}

#[test]
fn captures_debug_info_warn_and_error() {
    let guard = TestGuard::get();
    guard.with_test_subscriber(|| {
        debug!("debug");
        info!("info");
        warn!("warn");
        error!("error");
    });

    let mut subscription = LogState::get().subscribe();
    let messages: Vec<_> = std::mem::take(&mut subscription.backlog)
        .into_iter()
        .map(|log| log.message.clone())
        .collect();
    assert!(
        messages.iter().any(|message| message.contains("info")),
        "Missing info log in {messages:?}"
    );
    assert!(
        messages.iter().any(|message| message.contains("warn")),
        "Missing warn log in {messages:?}"
    );
    assert!(
        messages.iter().any(|message| message.contains("error")),
        "Missing error log in {messages:?}"
    );
}

#[test]
fn includes_span_context_and_source_location() {
    let guard = TestGuard::get();
    guard.with_test_subscriber(|| {
        let span = info_span!("The span", config_file_path = "/tmp/config.toml");
        let _entered = span.enter();
        info!("The message");
    });

    let subscription = LogState::get().subscribe();
    let message = &subscription.backlog.front().expect("log").message;
    assert_eq!(
        message,
        r#"The span: The message config_file_path="/tmp/config.toml""#
    );
}

#[test]
fn keeps_only_the_newest_twenty_logs() {
    let guard = TestGuard::get();
    guard.with_test_subscriber(|| {
        for index in 0..=25 {
            info!("event {index}");
        }
    });

    let subscription = LogState::get().subscribe();
    assert_eq!(subscription.backlog.len(), 20);
    let first = subscription.backlog.front().expect("first");
    assert!(first.message == "event 6", "{first:?}");
    let last = subscription.backlog.back().expect("last");
    assert!(last.message == "event 25", "{last:?}");
}

#[tokio::test]
async fn replays_backlog_before_live_events() {
    let guard = TestGuard::get();
    guard.with_test_subscriber(|| {
        info!("before subscribe");
    });

    let mut subscription = LogState::get().subscribe();
    assert_eq!(subscription.backlog.len(), 1);

    let first = subscription.backlog.front().expect("first");
    assert!(first.message == "before subscribe", "Got {first:?}");

    guard.with_test_subscriber(|| {
        info!("after subscribe");
    });

    let live = tokio::time::timeout(Duration::from_secs(1), subscription.receiver.recv())
        .map(|result| result.expect("timeout").expect("event"))
        .await;
    assert!(live.message == "after subscribe", "Got {live:?}");
}
