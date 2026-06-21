#[path = "support/daemon.rs"]
mod daemon_support;
#[path = "support/notifications.rs"]
mod notifications;

use canopa_notify::{
    AppPermissionControl, ControlCommand, ControlReply, DeliveryOutcome, NotificationAction,
    NotificationInput, NotifyControl,
};
use notifications::{notification, notification_with_body};
use std::time::Duration;

#[tokio::test]
async fn notify_control_reports_daemon_status() {
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());

    let reply = control.execute(ControlCommand::Status).await.unwrap();

    assert_eq!(
        reply,
        ControlReply::Status(canopa_notify::ControlStatus { available: true })
    );
}

#[tokio::test]
async fn notify_control_returns_active_notifications() {
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());

    let receipt = daemon
        .receive_notification(notification("Mail", "Inbox"))
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
    assert_eq!(active[0].accepted_notification.app_identity, "Mail");
}

#[tokio::test]
async fn notify_control_builds_notification_center_projection() {
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());

    daemon
        .receive_notification(NotificationInput {
            actions: vec![NotificationAction::new("open", "Open")],
            ..notification_with_body("Mail", "Inbox", "One unread message")
        })
        .await
        .unwrap();
    daemon
        .receive_notification(notification("Chat", "Message"))
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
async fn notify_control_subscription_emits_complete_replacement_projections() {
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());
    let mut subscription = control.subscribe_notification_state();

    let initial = subscription.next_projection().await.unwrap().unwrap();
    assert!(initial.groups.is_empty());

    daemon
        .receive_notification(notification("Mail", "Inbox"))
        .await
        .unwrap();

    let projection = tokio::time::timeout(Duration::from_secs(1), subscription.next_projection())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(projection.groups.len(), 1);
    assert_eq!(projection.groups[0].app_identity, "Mail");
    assert_eq!(projection.groups[0].items[0].summary, "Inbox");

    daemon
        .receive_notification(notification("Chat", "Message"))
        .await
        .unwrap();

    let projection = tokio::time::timeout(Duration::from_secs(1), subscription.next_projection())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(projection.groups.len(), 2);
    assert!(
        projection
            .groups
            .iter()
            .any(|group| group.app_identity == "Mail")
    );
    assert!(
        projection
            .groups
            .iter()
            .any(|group| group.app_identity == "Chat")
    );
}

#[tokio::test]
async fn notify_control_projects_app_permission_from_user_rule_store() {
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());

    daemon
        .receive_notification(notification("Chat", "Message"))
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
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());

    let receipt = daemon
        .receive_notification(NotificationInput {
            actions: vec![NotificationAction::new("open", "Open")],
            ..notification("Chat", "Message")
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
async fn sender_event_subscription_receives_action_invocation_events() {
    let daemon = daemon_support::open_headless_daemon().await;
    let mut events = daemon.subscribe_sender_events();

    let receipt = daemon
        .receive_notification(NotificationInput {
            actions: vec![NotificationAction::new("open", "Open")],
            ..notification("Chat", "Message")
        })
        .await
        .unwrap();

    daemon
        .invoke_action(receipt.notification_id, "open")
        .await
        .unwrap();

    let action_event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(action_event.notification_id, receipt.notification_id);
    assert_eq!(action_event.action_key.as_deref(), Some("open"));

    let close_event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(close_event.closed_id, Some(receipt.notification_id));
}

#[tokio::test]
async fn notify_control_clears_active_notifications() {
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());

    daemon
        .receive_notification(notification("Mail", "Inbox"))
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
