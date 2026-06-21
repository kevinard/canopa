#[path = "support/daemon.rs"]
mod daemon_support;
#[path = "support/notifications.rs"]
mod notifications;

use canopa_notify::{NotificationInput, Urgency};
use notifications::notification;

#[tokio::test]
async fn replacement_revises_existing_thread_and_can_create_banner_opportunity() {
    let daemon = daemon_support::open_headless_daemon().await;

    for index in 0..3 {
        daemon
            .receive_notification(NotificationInput {
                urgency: Urgency::Critical,
                ..notification("Chat", format!("Visible {index}"))
            })
            .await
            .unwrap();
    }

    let hidden = daemon
        .receive_notification(NotificationInput {
            progress: Some(10),
            ..notification("Downloader", "Download")
        })
        .await
        .unwrap();

    assert!(
        !daemon
            .visible_banners()
            .await
            .iter()
            .any(|banner| banner.notification_id == hidden.notification_id)
    );

    let replacement = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(hidden.notification_id),
            body: "Complete".to_string(),
            progress: Some(100),
            urgency: Urgency::Critical,
            ..notification("Downloader", "Download")
        })
        .await
        .unwrap();

    assert_eq!(replacement.notification_id, hidden.notification_id);

    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.history.len(), 4);
    let thread = snapshot
        .history
        .iter()
        .find(|thread| thread.id == hidden.notification_id)
        .unwrap();
    assert_eq!(thread.revision, 2);
    assert_eq!(
        thread.accepted_notification.replaces_id,
        Some(hidden.notification_id)
    );
    assert_eq!(thread.accepted_notification.body, "Complete");
    assert_eq!(thread.accepted_notification.progress, Some(100));
    assert!(
        daemon
            .visible_banners()
            .await
            .iter()
            .any(|banner| banner.notification_id == hidden.notification_id)
    );
}

#[tokio::test]
async fn repeated_fixed_replacement_id_updates_prior_thread() {
    let daemon = daemon_support::open_headless_daemon().await;

    let first = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(9999),
            body: "0%".to_string(),
            progress: Some(0),
            ..notification("notify-send", "Test Progress")
        })
        .await
        .unwrap();

    let replacement = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(9999),
            body: "5%".to_string(),
            progress: Some(5),
            ..notification("notify-send", "Test Progress")
        })
        .await
        .unwrap();

    assert_eq!(replacement.notification_id, first.notification_id);

    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.history.len(), 1);
    assert_eq!(snapshot.active.len(), 1);
    assert_eq!(snapshot.active[0].revision, 2);
    assert_eq!(snapshot.active[0].accepted_notification.body, "5%");
    assert_eq!(snapshot.active[0].accepted_notification.progress, Some(5));

    let banners = daemon.visible_banners().await;
    assert_eq!(banners.len(), 1);
    assert_eq!(banners[0].notification_id, first.notification_id);
    assert_eq!(banners[0].body, "5%");
    assert_eq!(banners[0].progress, Some(5));
}
