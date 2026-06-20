use canopa_notify::{Daemon, DaemonOptions, NotificationInput, Urgency};

#[tokio::test]
async fn replacement_revises_existing_thread_and_can_create_banner_opportunity() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();

    for index in 0..3 {
        daemon
            .receive_notification(NotificationInput {
                app_name: "Chat".to_string(),
                summary: format!("Visible {index}"),
                urgency: Urgency::Critical,
                ..NotificationInput::default()
            })
            .await
            .unwrap();
    }

    let hidden = daemon
        .receive_notification(NotificationInput {
            app_name: "Downloader".to_string(),
            summary: "Download".to_string(),
            progress: Some(10),
            ..NotificationInput::default()
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
            app_name: "Downloader".to_string(),
            replaces_id: Some(hidden.notification_id),
            summary: "Download".to_string(),
            body: "Complete".to_string(),
            progress: Some(100),
            urgency: Urgency::Critical,
            ..NotificationInput::default()
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
