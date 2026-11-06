use super::*;
use crate::print::test_bus::Bus;

#[tokio::test]
async fn tokens_are_one_use_and_a_wrong_native_client_cannot_steal_or_consume_them() {
    let bus = Bus::new().await;
    let frontend = bus.frontend().await;
    let first = bus.connect().await;
    let second = bus.connect().await;
    let owner = bus.owner(&frontend, &first, "").await;
    let wrong = bus.owner(&frontend, &second, "").await;
    let tokens = Tokens::default();
    let token = tokens
        .issue(owner.clone(), "approved printer")
        .await
        .unwrap();
    assert_ne!(token, 0);
    assert_eq!(tokens.consume(&wrong, token).await.unwrap(), None);
    assert_eq!(tokens.consume(&owner, 0).await.unwrap(), None);
    let later = bus.owner(&frontend, &first, "").await;
    assert_eq!(
        tokens.consume(&later, token).await.unwrap(),
        Some("approved printer")
    );
    assert_eq!(tokens.consume(&owner, token).await.unwrap(), None);
}

#[tokio::test]
async fn changed_app_id_and_direct_frontend_impersonation_do_not_authorize_tokens() {
    let bus = Bus::new().await;
    let frontend = bus.frontend().await;
    let client = bus.connect().await;
    let owner = bus.owner(&frontend, &client, "example.App").await;
    let wrong_app = bus.owner(&frontend, &client, "example.Other").await;
    let forged = bus.owner(&client, &client, "example.App").await;
    let tokens = Tokens::default();
    let token = tokens.issue(owner.clone(), 42).await.unwrap();
    assert_eq!(tokens.consume(&wrong_app, token).await.unwrap(), None);
    assert_eq!(tokens.consume(&forged, token).await.unwrap(), None);
    assert_eq!(tokens.consume(&owner, token).await.unwrap(), Some(42));
}

#[tokio::test]
async fn concurrent_print_calls_can_take_an_approval_only_once() {
    let bus = Bus::new().await;
    let peer = bus.connect().await;
    let owner = bus.owner(&peer, &peer, "").await;
    let tokens = Tokens::default();
    let token = tokens.issue(owner.clone(), 42).await.unwrap();
    let (first, second) =
        tokio::join!(tokens.consume(&owner, token), tokens.consume(&owner, token));
    let results = [first.unwrap(), second.unwrap()];
    assert_eq!(
        results.iter().filter(|value| **value == Some(42)).count(),
        1
    );
    assert_eq!(results.iter().filter(|value| value.is_none()).count(), 1);
}

#[tokio::test]
async fn expired_tokens_release_owned_resources_and_require_fresh_consent() {
    use std::{io::Read, os::unix::net::UnixStream};
    let bus = Bus::new().await;
    let peer = bus.connect().await;
    let owner = bus.owner(&peer, &peer, "").await;
    let tokens = Tokens::default();
    let (mut reader, writer) = UnixStream::pair().unwrap();
    reader
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let token = tokens.issue(owner.clone(), writer).await.unwrap();
    tokens
        .entries
        .lock()
        .unwrap()
        .get_mut(&token)
        .unwrap()
        .expires = Instant::now();
    assert!(tokens.consume(&owner, token).await.unwrap().is_none());
    assert_eq!(reader.read(&mut [0]).unwrap(), 0);
}

#[tokio::test]
async fn frontend_loss_cannot_reuse_approved_settings_after_reacquisition() {
    let bus = Bus::new().await;
    let frontend = bus.frontend().await;
    let client = bus.connect().await;
    let owner = bus.owner(&frontend, &client, "").await;
    let tokens = Tokens::default();
    let token = tokens.issue(owner.clone(), 42).await.unwrap();
    frontend
        .release_name("org.freedesktop.portal.Desktop")
        .await
        .unwrap();
    frontend
        .request_name("org.freedesktop.portal.Desktop")
        .await
        .unwrap();
    let later = bus.owner(&frontend, &client, "").await;
    assert_eq!(tokens.consume(&later, token).await.unwrap(), None);
    assert!(tokens.issue(owner, 43).await.is_err());
    let fresh = tokens.issue(later.clone(), 44).await.unwrap();
    assert_eq!(tokens.consume(&later, fresh).await.unwrap(), Some(44));
}

#[tokio::test]
async fn approval_capacity_is_bounded_globally_and_per_original_owner() {
    let bus = Bus::new().await;
    let tokens = Tokens::default();
    let mut peers = Vec::new();
    for _ in 0..8 {
        peers.push(bus.connect().await);
    }
    for (identity, peer) in peers.iter().enumerate() {
        let owner = bus
            .owner(peer, peer, &format!("example.App{identity}"))
            .await;
        let mut previous = Vec::new();
        for value in 0..PER_OWNER {
            let token = tokens.issue(owner.clone(), value).await.unwrap();
            assert!(!previous.contains(&token));
            previous.push(token);
        }
        assert!(matches!(
            tokens.issue(owner, 9).await,
            Err(fdo::Error::LimitsExceeded(_))
        ));
        let changed_app_id = bus.owner(peer, peer, "example.ChangedId").await;
        assert!(matches!(
            tokens.issue(changed_app_id, 9).await,
            Err(fdo::Error::LimitsExceeded(_))
        ));
    }
    assert_eq!(tokens.entries.lock().unwrap().len(), CAPACITY);
    let peer = bus.connect().await;
    let another = bus.owner(&peer, &peer, "example.OneMore").await;
    assert!(matches!(
        tokens.issue(another, 0).await,
        Err(fdo::Error::LimitsExceeded(_))
    ));
}

async fn eof(reader: std::os::unix::net::UnixStream) {
    use tokio::io::AsyncReadExt;
    reader.set_nonblocking(true).unwrap();
    let mut reader = tokio::net::UnixStream::from_std(reader).unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), reader.read(&mut [0]))
            .await
            .unwrap()
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn expiry_retires_descriptors_without_waiting_for_another_method_call() {
    let bus = Bus::new().await;
    let peer = bus.connect().await;
    let owner = bus.owner(&peer, &peer, "").await;
    let tokens = Tokens::default();
    let (reader, writer) = std::os::unix::net::UnixStream::pair().unwrap();
    tokens
        .issue_with_lifetime(owner, writer, Duration::from_millis(20))
        .await
        .unwrap();
    eof(reader).await;
    assert!(tokens.entries.lock().unwrap().is_empty());
}

#[tokio::test]
async fn original_client_loss_and_store_shutdown_release_retained_descriptors() {
    let bus = Bus::new().await;
    let frontend = bus.frontend().await;
    let client = bus.connect().await;
    let owner = bus.owner(&frontend, &client, "").await;
    let tokens = Tokens::default();
    let (reader, writer) = std::os::unix::net::UnixStream::pair().unwrap();
    tokens.issue(owner, writer).await.unwrap();
    client.close().await.unwrap();
    eof(reader).await;
    assert!(tokens.entries.lock().unwrap().is_empty());

    let peer = bus.connect().await;
    let owner = bus.owner(&peer, &peer, "").await;
    let (reader, writer) = std::os::unix::net::UnixStream::pair().unwrap();
    tokens.issue(owner, writer).await.unwrap();
    drop(tokens);
    eof(reader).await;
}

#[tokio::test]
async fn policy_cancellation_and_source_loss_retire_resources_without_another_call() {
    let bus = Bus::new().await;
    let peer = bus.connect().await;
    let owner = bus.owner(&peer, &peer, "").await;
    for closed in [false, true] {
        let tokens = Tokens::default();
        let (cancel, cancelled) = watch::channel(false);
        let (reader, writer) = std::os::unix::net::UnixStream::pair().unwrap();
        let token = tokens
            .issue_cancellable(owner.clone(), writer, cancelled)
            .await
            .unwrap();
        if !closed {
            cancel.send_replace(true);
        }
        drop(cancel);
        eof(reader).await;
        assert!(tokens.consume(&owner, token).await.unwrap().is_none());
    }
}

#[tokio::test]
async fn cancellation_is_checked_before_issuing_or_consuming_even_before_retirement_runs() {
    let bus = Bus::new().await;
    let peer = bus.connect().await;
    let owner = bus.owner(&peer, &peer, "").await;
    let tokens = Tokens::default();
    let (cancel, cancelled) = watch::channel(false);
    let token = tokens
        .issue_cancellable(owner.clone(), 42, cancelled.clone())
        .await
        .unwrap();
    cancel.send_replace(true);
    assert_eq!(tokens.consume(&owner, token).await.unwrap(), None);
    assert!(
        tokens
            .issue_cancellable(owner, 43, cancelled)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn old_guards_and_retirement_cannot_remove_a_later_issuance_with_the_same_wire_number() {
    let bus = Bus::new().await;
    let peer = bus.connect().await;
    let owner = bus.owner(&peer, &peer, "").await;
    let tokens = Tokens::default();
    let (_cancel, cancelled) = watch::channel(false);
    let old = tokens
        .issue_retained(owner.clone(), 1, cancelled.clone())
        .await
        .unwrap();
    let number = old.token();
    let old_issuance = old.issuance.clone();
    assert_eq!(tokens.consume(&owner, number).await.unwrap(), Some(1));
    let later = tokens
        .issue_cancellable(owner.clone(), 2, cancelled)
        .await
        .unwrap();
    // Force the normally random reuse so the regression is deterministic.
    {
        let mut entries = tokens.entries.lock().unwrap();
        let replacement = entries.remove(&later).unwrap();
        entries.insert(number, replacement);
    }
    drop(old);
    retire(&tokens.entries, number, &old_issuance);
    assert_eq!(tokens.consume(&owner, number).await.unwrap(), Some(2));
}
