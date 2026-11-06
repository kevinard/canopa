use crate::daemon_support;
use crate::notifications;

use canopa_notify::{
    ControlCommand, ControlReply, NotificationAction, NotificationInput, NotifyControl, Urgency,
};
use notifications::notification;

#[tokio::test]
async fn replacement_reads_only_its_target_and_preserves_target_read_errors() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("history.db");
    let daemon =
        canopa_notify::Daemon::open(canopa_notify::DaemonOptions::headless(database.clone()))
            .await
            .unwrap();
    let target = daemon
        .receive_notification(notification("Mail", "Target"))
        .await
        .unwrap();
    let unrelated = daemon
        .receive_notification(notification("Chat", "Unrelated"))
        .await
        .unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&database))
        .await
        .unwrap();
    // Fault injection at the storage boundary; observations remain through Daemon.
    sqlx::query("UPDATE notification_threads SET actions_json = 'invalid' WHERE id = ?1")
        .bind(unrelated.notification_id.get())
        .execute(&pool)
        .await
        .unwrap();
    let updated = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(target.notification_id),
            ..notification("Mail", "Updated")
        })
        .await
        .expect("an unrelated unreadable active row must not block replacement");
    assert_eq!(updated.notification_id, target.notification_id);
    assert!(
        daemon
            .visible_banners()
            .await
            .iter()
            .any(|banner| banner.notification_id == target.notification_id
                && banner.summary == "Updated")
    );
    assert!(
        daemon
            .receive_notification(NotificationInput {
                replaces_id: Some(unrelated.notification_id),
                ..notification("Chat", "Must fail")
            })
            .await
            .is_err(),
        "an unreadable target must not be treated as absent"
    );
    pool.close().await;
    daemon.shutdown().await.unwrap();
}

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
async fn unresolved_internal_replacement_ids_do_not_form_transport_aliases() {
    let daemon = daemon_support::open_headless_daemon().await;

    let first = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(canopa_notify::InternalNotificationId::new(9999).unwrap()),
            body: "0%".to_string(),
            progress: Some(0),
            ..notification("notify-send", "Test Progress")
        })
        .await
        .unwrap();

    let replacement = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(canopa_notify::InternalNotificationId::new(9999).unwrap()),
            body: "5%".to_string(),
            progress: Some(5),
            ..notification("notify-send", "Test Progress")
        })
        .await
        .unwrap();

    assert_ne!(replacement.notification_id, first.notification_id);

    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.history.len(), 2);
    assert_eq!(snapshot.active.len(), 2);
    assert!(snapshot.active.iter().all(|thread| thread.revision == 1));

    let banners = daemon.visible_banners().await;
    assert_eq!(banners.len(), 2);
    assert!(banners.iter().any(
        |banner| banner.notification_id == replacement.notification_id && banner.body == "5%"
    ));
}

#[tokio::test]
async fn replacing_closed_history_creates_a_new_live_thread() {
    let daemon = daemon_support::open_headless_daemon().await;
    let first = daemon
        .receive_notification(notification("Mail", "Archived"))
        .await
        .unwrap();
    daemon
        .close_notification(first.notification_id)
        .await
        .unwrap();
    let next = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(first.notification_id),
            ..notification("Mail", "New")
        })
        .await
        .unwrap();
    assert_ne!(first.notification_id, next.notification_id);
    let snapshot = daemon.snapshot().await.unwrap();
    let old = snapshot
        .history
        .iter()
        .find(|thread| thread.id == first.notification_id)
        .unwrap();
    assert_eq!(old.status, canopa_notify::NotificationStatus::Closed);
    assert_eq!(old.accepted_notification.summary, "Archived");
}

#[tokio::test]
async fn replacement_replaces_actions_and_rejects_stale_surface_invocations() {
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());

    let first = daemon
        .receive_notification(NotificationInput {
            actions: vec![
                NotificationAction::new("open", "Open"),
                NotificationAction::new("archive", "Archive"),
            ],
            ..notification("Mail", "Inbox")
        })
        .await
        .unwrap();

    let replacement = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(first.notification_id),
            actions: vec![NotificationAction::new("reply", "Open")],
            ..notification("Mail", "Inbox updated")
        })
        .await
        .unwrap();

    assert_eq!(replacement.notification_id, first.notification_id);

    let snapshot = daemon.snapshot().await.unwrap();
    let thread = snapshot
        .active
        .iter()
        .find(|thread| thread.id == first.notification_id)
        .unwrap();
    assert_eq!(
        thread
            .accepted_notification
            .actions
            .iter()
            .map(|action| action.key.as_str())
            .collect::<Vec<_>>(),
        vec!["reply"]
    );
    assert_eq!(
        daemon.visible_banners().await[0]
            .actions
            .iter()
            .map(|action| (action.key.as_str(), action.label.as_str()))
            .collect::<Vec<_>>(),
        vec![("reply", "Open")]
    );

    let ControlReply::NotificationCenterProjection(projection) = control
        .execute(ControlCommand::NotificationCenterProjection)
        .await
        .unwrap()
    else {
        panic!("expected notification center projection reply");
    };
    assert_eq!(
        projection.groups[0].items[0]
            .actions
            .iter()
            .map(|action| (action.key.as_str(), action.label.as_str()))
            .collect::<Vec<_>>(),
        vec![("reply", "Open")]
    );

    assert!(
        control
            .execute(ControlCommand::InvokeNotificationAction {
                notification_id: first.notification_id,
                action_key: "open".to_string(),
            })
            .await
            .is_err()
    );
    assert!(
        control
            .execute(ControlCommand::InvokeNotificationAction {
                notification_id: first.notification_id,
                action_key: "Open".to_string(),
            })
            .await
            .is_err()
    );
    assert_eq!(
        control
            .execute(ControlCommand::InvokeNotificationAction {
                notification_id: first.notification_id,
                action_key: "reply".to_string(),
            })
            .await
            .unwrap(),
        ControlReply::CommandApplied
    );
}
