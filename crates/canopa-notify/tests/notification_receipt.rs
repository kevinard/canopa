#[path = "support/daemon.rs"]
mod daemon_support;
#[path = "support/notifications.rs"]
mod notifications;

use canopa_notify::{
    Daemon, DaemonOptions, NotificationImageData, NotificationInput, NotificationTimeout,
    NotificationVisualReference, SenderCloseReason, SenderEvent, Urgency,
};
use notifications::{blank_notification, notification_with_body};

#[tokio::test]
async fn decoding_an_unresolved_wire_id_cannot_replace_an_internal_thread() {
    let daemon = daemon_support::open_headless_daemon().await;
    let original = daemon
        .receive_notification(notification_with_body("Original", "Keep me", ""))
        .await
        .unwrap();
    let request = canopa_notify::NotificationServerRequest {
        app_name: "Other sender".into(),
        replaces_id: u32::try_from(original.notification_id.get()).unwrap(),
        app_icon: String::new(),
        summary: "Separate notification".into(),
        body: String::new(),
        actions: Vec::new(),
        hints: std::collections::HashMap::new(),
        expire_timeout: 0,
    };
    let received = daemon
        .receive_notification(request.into_notification_input())
        .await
        .unwrap();
    assert_ne!(
        received.notification_id, original.notification_id,
        "request decoding treated a wire ID as an internal replacement target"
    );
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.active.len(), 2);
    let original = snapshot
        .active
        .iter()
        .find(|thread| thread.id == original.notification_id)
        .unwrap();
    assert_eq!(original.revision, 1);
    assert_eq!(original.accepted_notification.summary, "Keep me");
}

#[tokio::test]
async fn completed_notification_churn_keeps_only_bounded_diagnostics() {
    let daemon = daemon_support::open_headless_daemon().await;
    for _ in 0..600 {
        let receipt = daemon
            .receive_notification(NotificationInput {
                transient: true,
                expire_timeout: NotificationTimeout::Never,
                ..notification_with_body("Churn", "One event", "")
            })
            .await
            .unwrap();
        daemon
            .close_notification(receipt.notification_id)
            .await
            .unwrap();
    }
    assert!(daemon.sender_events().await.len() <= 256);
    assert!(daemon.delivery_decision_logs().await.len() <= 256);
    assert!(daemon.visible_banners().await.is_empty());
    assert!(daemon.snapshot().await.unwrap().history.is_empty());
}

#[test]
fn raw_image_channels_are_validated_before_narrowing() {
    for channels in [-253, -252, 259, 260, i32::MAX, i32::MIN] {
        for alpha in [false, true] {
            assert!(
                NotificationImageData::from_freedesktop(1, 1, 4, alpha, 8, channels, vec![0; 4])
                    .is_none(),
                "accepted channel count {channels}"
            );
        }
    }
    assert!(
        NotificationImageData::from_freedesktop(1, 2, 4, false, 8, 3, vec![0; 7]).is_some(),
        "final-row padding is optional"
    );
}

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
    let mut events = daemon.subscribe_sender_events();

    let receipt = daemon
        .receive_notification(blank_notification("Blank Sender"))
        .await
        .unwrap();

    assert_eq!(receipt.delivery_outcome.as_str(), "dropped");
    assert_ne!(receipt.notification_id.get(), 0);
    assert_eq!(
        events.recv().await.unwrap(),
        SenderEvent::NotificationClosed {
            notification_id: receipt.notification_id,
            reason: SenderCloseReason::Undefined,
        }
    );
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
async fn transient_notification_never_enters_history_or_survives_restart() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    let mut events = daemon.subscribe_sender_events();

    let receipt = daemon
        .receive_notification(NotificationInput {
            transient: true,
            expire_timeout: NotificationTimeout::Never,
            ..notification_with_body("Transient App", "Status", "Runtime only")
        })
        .await
        .unwrap();

    assert!(receipt.notification_id.is_runtime_only());
    assert!(daemon.snapshot().await.unwrap().active.is_empty());
    assert!(daemon.snapshot().await.unwrap().history.is_empty());
    daemon
        .close_notification(receipt.notification_id)
        .await
        .unwrap();
    assert_eq!(
        events.recv().await.unwrap(),
        SenderEvent::NotificationClosed {
            notification_id: receipt.notification_id,
            reason: SenderCloseReason::Closed,
        }
    );
    drop(daemon);

    let restarted = Daemon::open(DaemonOptions::headless(database))
        .await
        .unwrap();
    assert!(restarted.snapshot().await.unwrap().history.is_empty());
}

#[tokio::test]
async fn category_and_progress_are_persisted_without_image_payloads() {
    let daemon = daemon_support::open_headless_daemon().await;

    daemon
        .receive_notification(NotificationInput {
            category: Some("transfer".to_string()),
            progress: Some(95),
            image_data: Some(test_image_data()),
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

#[tokio::test]
async fn image_only_raw_notifications_are_accepted_without_persisting_payloads() {
    let daemon = daemon_support::open_headless_daemon().await;

    let receipt = daemon
        .receive_notification(NotificationInput {
            app_name: "Photos".to_string(),
            image_data: Some(test_image_data()),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    assert_eq!(receipt.delivery_outcome.as_str(), "shown");
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.active.len(), 1);
    assert_eq!(
        snapshot.active[0].accepted_notification.app_identity,
        "Photos"
    );
    assert!(!snapshot.active[0].accepted_notification.has_image_payload);
}

#[tokio::test]
async fn visual_references_are_persisted_as_accepted_notification_facts() {
    let daemon = daemon_support::open_headless_daemon().await;

    daemon
        .receive_notification(NotificationInput {
            app_icon: NotificationVisualReference::icon_name("mail-unread"),
            image_reference: NotificationVisualReference::local_path("/tmp/screenshot.jpg"),
            ..notification_with_body("Mail", "Inbox", "One unread message")
        })
        .await
        .unwrap();

    let snapshot = daemon.snapshot().await.unwrap();
    let notification = &snapshot.active[0].accepted_notification;
    assert_eq!(
        notification.app_icon,
        NotificationVisualReference::icon_name("mail-unread")
    );
    assert_eq!(
        notification.image_reference,
        NotificationVisualReference::local_path("/tmp/screenshot.jpg")
    );
}

fn test_image_data() -> NotificationImageData {
    NotificationImageData::from_freedesktop(1, 1, 4, true, 8, 4, vec![255, 0, 0, 255]).unwrap()
}
