use canopa_notify::{Daemon, DaemonOptions, NotificationInput, Urgency};

#[tokio::test]
async fn default_allowed_notification_is_persisted_as_an_active_thread() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();

    let receipt = daemon
        .receive_notification(NotificationInput {
            app_name: "Example Mail".to_string(),
            desktop_entry: Some("example-mail.desktop".to_string()),
            summary: "Inbox".to_string(),
            body: "One unread message".to_string(),
            urgency: Urgency::Normal,
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    assert_eq!(receipt.delivery_outcome.as_str(), "shown");

    let snapshot = daemon.snapshot().await.unwrap();

    assert_eq!(snapshot.active.len(), 1);
    let thread = &snapshot.active[0];
    assert_eq!(thread.id, receipt.notification_id);
    assert_eq!(thread.app_identity, "example-mail.desktop");
    assert_eq!(thread.summary, "Inbox");
    assert_eq!(thread.body, "One unread message");
    assert_eq!(thread.urgency, Urgency::Normal);
    assert_eq!(thread.status.as_str(), "active");
}

#[tokio::test]
async fn markup_body_is_rendered_as_plain_text_while_raw_body_is_retained() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();

    daemon
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Message".to_string(),
            body: "<b>Hello</b><br/>World &amp; friends".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    let snapshot = daemon.snapshot().await.unwrap();
    let thread = &snapshot.active[0];
    assert_eq!(thread.body, "Hello\nWorld & friends");
    assert_eq!(thread.raw_body, "<b>Hello</b><br/>World &amp; friends");
}

#[tokio::test]
async fn empty_noop_notifications_are_dropped_without_history() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();

    let receipt = daemon
        .receive_notification(NotificationInput {
            app_name: "Blank Sender".to_string(),
            summary: "   ".to_string(),
            body: "\n\t".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    assert_eq!(receipt.delivery_outcome.as_str(), "dropped");
    assert_eq!(daemon.snapshot().await.unwrap().history.len(), 0);
}

#[tokio::test]
async fn category_and_progress_are_persisted_without_image_payloads() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();

    daemon
        .receive_notification(NotificationInput {
            app_name: "Downloader".to_string(),
            summary: "Download".to_string(),
            body: "Almost done".to_string(),
            category: Some("transfer".to_string()),
            progress: Some(95),
            image_data: Some(vec![1, 2, 3, 4]),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    let snapshot = daemon.snapshot().await.unwrap();
    let thread = &snapshot.active[0];
    assert_eq!(thread.category.as_deref(), Some("transfer"));
    assert_eq!(thread.progress, Some(95));
    assert!(!thread.has_image_payload);
}
