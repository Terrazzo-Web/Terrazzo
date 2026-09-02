#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::extract::ws;
use tokio::sync::mpsc;
use trz_gateway_common::p2p::protocol::FailureCode;
use trz_gateway_common::p2p::protocol::P2pConnectionId;
use trz_gateway_common::p2p::protocol::SessionDescription;
use trz_gateway_common::p2p::protocol::SignalMessage;

use super::REGISTRATION_WAIT_TIMEOUT;
use super::SIGNAL_QUEUE_CAPACITY;
use super::Signaling;
use super::parse_message;
use super::registration::Registration;
use super::valid_client_message;
use super::valid_server_message;

fn registration(signaling: &Signaling, name: &str) -> Arc<Registration> {
    let (sender, _receiver) = mpsc::channel(SIGNAL_QUEUE_CAPACITY);
    signaling.install_registration(name.into(), sender)
}

#[tokio::test]
async fn replacement_cancels_old_sessions_and_stale_cleanup_preserves_replacement() {
    let signaling = Signaling::default();
    let old = registration(&signaling, "server");
    let mut session = old.create_session().unwrap();
    let replacement = registration(&signaling, "server");

    assert!(!old.active.load(Ordering::Acquire));
    assert_eq!(
        FailureCode::PeerDisconnected,
        match session.incoming.recv().await.unwrap() {
            SignalMessage::Failure { code, .. } => code,
            message => panic!("unexpected message: {message:?}"),
        }
    );
    signaling.remove_registration(&"server".into(), &old);
    let current = signaling
        .state
        .lock()
        .unwrap()
        .registrations
        .get("server")
        .unwrap()
        .clone();
    assert!(Arc::ptr_eq(&replacement, &current));
}

#[tokio::test]
async fn connect_before_register_wakes_and_cancelled_waiter_is_removed() {
    let signaling = Arc::new(Signaling::default());
    let waiter = tokio::spawn({
        let signaling = signaling.clone();
        async move { signaling.wait_for_registration("later".into()).await }
    });
    tokio::task::yield_now().await;
    assert_eq!(1, signaling.state.lock().unwrap().waiters["later"].count);
    let expected = registration(&signaling, "later");
    assert!(Arc::ptr_eq(&expected, &waiter.await.unwrap().unwrap()));
    assert!(
        !signaling
            .state
            .lock()
            .unwrap()
            .waiters
            .contains_key("later")
    );

    let cancelled = tokio::spawn({
        let signaling = signaling.clone();
        async move { signaling.wait_for_registration("never".into()).await }
    });
    tokio::task::yield_now().await;
    cancelled.abort();
    let _ = cancelled.await;
    assert!(
        !signaling
            .state
            .lock()
            .unwrap()
            .waiters
            .contains_key("never")
    );
}

#[tokio::test(start_paused = true)]
async fn offline_wait_is_thirty_seconds() {
    let signaling = Arc::new(Signaling::default());
    let wait = tokio::time::timeout(
        REGISTRATION_WAIT_TIMEOUT,
        signaling.wait_for_registration("offline".into()),
    );
    assert!(wait.await.is_err());
    assert!(
        !signaling
            .state
            .lock()
            .unwrap()
            .waiters
            .contains_key("offline")
    );
}

#[tokio::test]
async fn routes_messages_and_cleans_up_disconnects() {
    let signaling = Signaling::default();
    let registration = registration(&signaling, "server");
    let mut session = registration.create_session().unwrap();
    let answer = SignalMessage::Description {
        connection_id: session.connection_id,
        description: SessionDescription::Answer("answer".into()),
    };
    registration.relay_to_client(answer.clone()).unwrap();
    assert_eq!(Some(answer), session.incoming.recv().await);
    registration.remove_session(session.connection_id);
    assert!(
        registration
            .relay_to_client(SignalMessage::EndOfCandidates {
                connection_id: session.connection_id,
            })
            .is_err()
    );
    assert_eq!(0, signaling.pending_sessions.load(Ordering::Acquire));
}

#[test]
fn rejects_malformed_or_wrong_direction_messages() {
    assert!(parse_message(ws::Message::Text("not json".into())).is_err());
    assert!(!valid_server_message(&SignalMessage::Description {
        connection_id: P2pConnectionId::new(),
        description: SessionDescription::Offer("offer".into()),
    }));
    assert!(!valid_client_message(&SignalMessage::Description {
        connection_id: P2pConnectionId::new(),
        description: SessionDescription::Answer("answer".into()),
    }));
}

#[tokio::test]
async fn shutdown_cancels_sessions_and_waiters() {
    let signaling = Arc::new(Signaling::default());
    let registration = registration(&signaling, "server");
    let mut session = registration.create_session().unwrap();
    let waiter = tokio::spawn({
        let signaling = signaling.clone();
        async move { signaling.wait_for_registration("offline".into()).await }
    });
    tokio::task::yield_now().await;
    signaling.shutdown();
    assert!(waiter.await.unwrap().is_err());
    assert!(matches!(
        session.incoming.recv().await,
        Some(SignalMessage::Failure {
            code: FailureCode::PeerDisconnected,
            ..
        })
    ));
    assert_eq!(0, signaling.pending_sessions.load(Ordering::Acquire));
}
