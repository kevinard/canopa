use canopa_notify::{
    AppPermissionControl, ControlCommand, ControlReply, Daemon, DaemonOptions, DeliveryOutcome,
    NotificationAction, NotificationInput, NotifyControl,
};

#[tokio::test]
async fn notify_control_reports_daemon_status() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let control = NotifyControl::new(daemon);

    let reply = control.execute(ControlCommand::Status).await.unwrap();

    assert_eq!(
        reply,
        ControlReply::Status(canopa_notify::ControlStatus { available: true })
    );
}

#[tokio::test]
async fn notify_control_returns_active_notifications() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let control = NotifyControl::new(daemon.clone());

    let receipt = daemon
        .receive_notification(NotificationInput {
            app_name: "Mail".to_string(),
            summary: "Inbox".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    let reply = control
        .execute(ControlCommand::ActiveNotifications)
        .await
        .unwrap();
    let ControlReply::ActiveNotifications(active) = reply else {
        panic!("expected active notifications reply");
    };

    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, receipt.notification_id);
    assert_eq!(active[0].app_identity, "Mail");
}

#[tokio::test]
async fn notify_control_builds_notification_center_projection() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let control = NotifyControl::new(daemon.clone());

    daemon
        .receive_notification(NotificationInput {
            app_name: "Mail".to_string(),
            summary: "Inbox".to_string(),
            body: "One unread message".to_string(),
            actions: vec![NotificationAction::new("open", "Open")],
            ..NotificationInput::default()
        })
        .await
        .unwrap();
    daemon
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Message".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    let reply = control
        .execute(ControlCommand::NotificationCenterProjection)
        .await
        .unwrap();
    let ControlReply::NotificationCenterProjection(projection) = reply else {
        panic!("expected notification center projection reply");
    };

    assert_eq!(projection.groups.len(), 2);

    let mail = projection
        .groups
        .iter()
        .find(|group| group.app_identity == "Mail")
        .unwrap();
    assert_eq!(mail.permission, AppPermissionControl::Allow);
    assert_eq!(mail.items.len(), 1);
    assert_eq!(mail.items[0].summary, "Inbox");
    assert_eq!(mail.items[0].body, "One unread message");
    assert_eq!(mail.items[0].actions[0].key, "open");

    let chat = projection
        .groups
        .iter()
        .find(|group| group.app_identity == "Chat")
        .unwrap();
    assert_eq!(chat.permission, AppPermissionControl::Allow);
    assert_eq!(chat.items[0].summary, "Message");
}

#[tokio::test]
async fn notify_control_projects_app_permission_from_user_rule_store() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let control = NotifyControl::new(daemon.clone());

    daemon
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Message".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();
    control
        .execute(ControlCommand::SetAppPermission {
            app_identity: "Chat".to_string(),
            permission: AppPermissionControl::Silence,
        })
        .await
        .unwrap();

    let reply = control
        .execute(ControlCommand::NotificationCenterProjection)
        .await
        .unwrap();
    let ControlReply::NotificationCenterProjection(projection) = reply else {
        panic!("expected notification center projection reply");
    };
    let chat = projection
        .groups
        .iter()
        .find(|group| group.app_identity == "Chat")
        .unwrap();

    assert_eq!(chat.permission, AppPermissionControl::Silence);
}

#[tokio::test]
async fn notify_control_applies_notification_center_interaction_commands() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let control = NotifyControl::new(daemon.clone());

    let receipt = daemon
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Message".to_string(),
            actions: vec![NotificationAction::new("open", "Open")],
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    assert_eq!(
        control
            .execute(ControlCommand::SetAppPermission {
                app_identity: "Chat".to_string(),
                permission: AppPermissionControl::Silence,
            })
            .await
            .unwrap(),
        ControlReply::CommandApplied
    );
    assert_eq!(
        daemon.snapshot().await.unwrap().active[0].delivery_outcome,
        DeliveryOutcome::Silenced
    );

    assert_eq!(
        control
            .execute(ControlCommand::InvokeNotificationAction {
                notification_id: receipt.notification_id,
                action_key: "open".to_string(),
            })
            .await
            .unwrap(),
        ControlReply::CommandApplied
    );
    assert_eq!(
        daemon.snapshot().await.unwrap().history[0].status.as_str(),
        "closed"
    );

    assert_eq!(
        control
            .execute(ControlCommand::DeleteNotificationHistory)
            .await
            .unwrap(),
        ControlReply::CommandApplied
    );
    assert_eq!(daemon.snapshot().await.unwrap().history.len(), 0);
}

#[tokio::test]
async fn notify_control_clears_active_notifications() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let control = NotifyControl::new(daemon.clone());

    daemon
        .receive_notification(NotificationInput {
            app_name: "Mail".to_string(),
            summary: "Inbox".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    assert_eq!(
        control
            .execute(ControlCommand::ClearActiveNotifications)
            .await
            .unwrap(),
        ControlReply::CommandApplied
    );

    let snapshot = daemon.snapshot().await.unwrap();
    assert!(snapshot.active.is_empty());
    assert_eq!(snapshot.history.len(), 1);
    assert_eq!(snapshot.history[0].status.as_str(), "dismissed");
}
