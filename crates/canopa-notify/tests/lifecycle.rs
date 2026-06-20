use canopa_notify::{Daemon, DaemonOptions, NotificationAction, NotificationInput};

#[tokio::test]
async fn actions_close_non_resident_notifications_but_resident_notifications_stay_active() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();

    let non_resident = daemon
        .receive_notification(NotificationInput {
            app_name: "Calendar".to_string(),
            summary: "Standup".to_string(),
            actions: vec![NotificationAction::new("open", "Open")],
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    daemon
        .invoke_action(non_resident.notification_id, "open")
        .await
        .unwrap();

    let resident = daemon
        .receive_notification(NotificationInput {
            app_name: "Music".to_string(),
            summary: "Playing".to_string(),
            resident: true,
            actions: vec![NotificationAction::new("pause", "Pause")],
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    daemon
        .invoke_action(resident.notification_id, "pause")
        .await
        .unwrap();

    let snapshot = daemon.snapshot().await.unwrap();
    let closed = snapshot
        .history
        .iter()
        .find(|thread| thread.id == non_resident.notification_id)
        .unwrap();
    let still_active = snapshot
        .history
        .iter()
        .find(|thread| thread.id == resident.notification_id)
        .unwrap();

    assert_eq!(closed.status.as_str(), "closed");
    assert_eq!(still_active.status.as_str(), "active");

    let events = daemon.sender_events().await;
    assert!(
        events
            .iter()
            .any(|event| event.action_key.as_deref() == Some("open"))
    );
    assert!(
        events
            .iter()
            .any(|event| event.closed_id == Some(non_resident.notification_id))
    );
}

#[tokio::test]
async fn active_notifications_can_be_dismissed_expired_closed_and_marked_stale_after_restart() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();

    let dismissed = daemon
        .receive_notification(NotificationInput {
            app_name: "Mail".to_string(),
            summary: "Dismiss me".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();
    let expired = daemon
        .receive_notification(NotificationInput {
            app_name: "Timer".to_string(),
            summary: "Expire me".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();
    let closed = daemon
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Close me".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();
    let stale = daemon
        .receive_notification(NotificationInput {
            app_name: "Build".to_string(),
            summary: "Still active".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    daemon
        .dismiss_notification(dismissed.notification_id)
        .await
        .unwrap();
    daemon
        .expire_notification(expired.notification_id)
        .await
        .unwrap();
    daemon
        .close_notification(closed.notification_id)
        .await
        .unwrap();
    drop(daemon);

    let restarted = Daemon::open(DaemonOptions::headless(database))
        .await
        .unwrap();
    let snapshot = restarted.snapshot().await.unwrap();

    assert_status(&snapshot, dismissed.notification_id, "dismissed");
    assert_status(&snapshot, expired.notification_id, "expired");
    assert_status(&snapshot, closed.notification_id, "closed");
    assert_status(&snapshot, stale.notification_id, "stale");
}

fn assert_status(snapshot: &canopa_notify::NotificationSnapshot, id: i64, status: &str) {
    let thread = snapshot
        .history
        .iter()
        .find(|thread| thread.id == id)
        .unwrap();
    assert_eq!(thread.status.as_str(), status);
}
