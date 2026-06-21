#[path = "support/daemon.rs"]
mod daemon_support;
#[path = "support/notifications.rs"]
mod notifications;

use canopa_notify::{NotificationInput, Urgency};
use notifications::{blank_notification, notification_with_body};

#[tokio::test]
async fn default_allowed_notification_is_persisted_as_an_active_thread() {
    let daemon = daemon_support::open_headless_daemon().await;

    let receipt = daemon
        .receive_notification(NotificationInput {
            desktop_entry: Some("example-mail.desktop".to_string()),
            urgency: Urgency::Normal,
            ..notification_with_body("Example Mail", "Inbox", "One unread message")
        })
        .await
        .unwrap();

    assert_eq!(receipt.delivery_outcome.as_str(), "shown");

    let snapshot = daemon.snapshot().await.unwrap();

    assert_eq!(snapshot.active.len(), 1);
    let thread = &snapshot.active[0];
    assert_eq!(thread.id, receipt.notification_id);
    assert_eq!(
        thread.accepted_notification.app_identity,
        "example-mail.desktop"
    );
    assert_eq!(thread.accepted_notification.summary, "Inbox");
    assert_eq!(thread.accepted_notification.body, "One unread message");
    assert_eq!(thread.accepted_notification.urgency, Urgency::Normal);
    assert_eq!(thread.status.as_str(), "active");
}

#[tokio::test]
async fn empty_noop_notifications_are_dropped_without_history() {
    let daemon = daemon_support::open_headless_daemon().await;

    let receipt = daemon
        .receive_notification(blank_notification("Blank Sender"))
        .await
        .unwrap();

    assert_eq!(receipt.delivery_outcome.as_str(), "dropped");
    assert_eq!(daemon.snapshot().await.unwrap().history.len(), 0);
}

#[tokio::test]
async fn empty_noop_notifications_bypass_delivery_policy() {
    let daemon = daemon_support::open_headless_daemon().await;

    let receipt = daemon
        .receive_notification(blank_notification("Blank Sender"))
        .await
        .unwrap();

    assert_eq!(receipt.delivery_outcome.as_str(), "dropped");
    assert!(daemon.delivery_decision_logs().await.is_empty());
}

#[tokio::test]
async fn category_and_progress_are_persisted_without_image_payloads() {
    let daemon = daemon_support::open_headless_daemon().await;

    daemon
        .receive_notification(NotificationInput {
            category: Some("transfer".to_string()),
            progress: Some(95),
            image_data: Some(vec![1, 2, 3, 4]),
            ..notification_with_body("Downloader", "Download", "Almost done")
        })
        .await
        .unwrap();

    let snapshot = daemon.snapshot().await.unwrap();
    let thread = &snapshot.active[0];
    assert_eq!(
        thread.accepted_notification.category.as_deref(),
        Some("transfer")
    );
    assert_eq!(thread.accepted_notification.progress, Some(95));
    assert!(!thread.accepted_notification.has_image_payload);
}
