#[path = "support/daemon.rs"]
mod daemon_support;
#[path = "support/notifications.rs"]
mod notifications;

use canopa_notify::{
    ActivationTokenFuture, ActivationTokenProvider, AppPermissionControl, ControlCommand,
    ControlReply, Daemon, DaemonOptions, DeliveryOutcome, NotificationAction, NotificationInput,
    NotifyControl, SenderEvent,
};
use notifications::{notification, notification_with_body};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug)]
struct FakeActivationTokenProvider {
    tokens: Arc<Mutex<VecDeque<String>>>,
    requested_app_identities: Arc<Mutex<Vec<String>>>,
    available: bool,
}

impl FakeActivationTokenProvider {
    fn new(tokens: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            tokens: Arc::new(Mutex::new(tokens.into_iter().map(Into::into).collect())),
            requested_app_identities: Arc::new(Mutex::new(Vec::new())),
            available: true,
        }
    }

    fn unavailable() -> Self {
        Self {
            tokens: Arc::new(Mutex::new(VecDeque::new())),
            requested_app_identities: Arc::new(Mutex::new(Vec::new())),
            available: false,
        }
    }

    fn requested_app_identities(&self) -> Vec<String> {
        self.requested_app_identities.lock().unwrap().clone()
    }
}

impl ActivationTokenProvider for FakeActivationTokenProvider {
    fn available(&self) -> bool {
        self.available
    }

    fn activation_token(&self, app_identity: String) -> ActivationTokenFuture {
        self.requested_app_identities
            .lock()
            .unwrap()
            .push(app_identity);
        let token = self.tokens.lock().unwrap().pop_front();
        Box::pin(async move { token })
    }
}

async fn open_daemon_with_activation_provider(
    provider: FakeActivationTokenProvider,
) -> (tempfile::TempDir, Daemon) {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db"))
            .with_activation_token_provider(provider),
    )
    .await
    .unwrap();
    (temp, daemon)
}

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
            ..notification_with_body("Mail", "Inbox", "<b>One</b> unread message")
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
    assert!(mail.items[0].body_markup.runs[0].style.bold);
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
async fn notification_center_projects_actions_only_for_active_items() {
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());

    daemon
        .receive_notification(NotificationInput {
            actions: vec![
                NotificationAction::new("default", "Open"),
                NotificationAction::new("reply", "Reply"),
                NotificationAction::new("archive", "Archive"),
                NotificationAction::new("mark_read", "Mark read"),
            ],
            ..notification("Mail", "Inbox")
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
    daemon
        .receive_notification(NotificationInput {
            actions: vec![NotificationAction::new("reply", "Reply")],
            ..notification("Chat", "Message")
        })
        .await
        .unwrap();

    let inactive = daemon
        .receive_notification(NotificationInput {
            actions: vec![NotificationAction::new("snooze", "Snooze")],
            ..notification("Calendar", "Standup")
        })
        .await
        .unwrap();
    daemon
        .dismiss_notification(inactive.notification_id)
        .await
        .unwrap();

    let reply = control
        .execute(ControlCommand::NotificationCenterProjection)
        .await
        .unwrap();
    let ControlReply::NotificationCenterProjection(projection) = reply else {
        panic!("expected notification center projection reply");
    };

    let mail = projection
        .groups
        .iter()
        .find(|group| group.app_identity == "Mail")
        .unwrap();
    assert_eq!(
        mail.items[0]
            .actions
            .iter()
            .map(|action| action.key.as_str())
            .collect::<Vec<_>>(),
        vec!["reply", "archive", "mark_read"]
    );

    let chat = projection
        .groups
        .iter()
        .find(|group| group.app_identity == "Chat")
        .unwrap();
    assert_eq!(chat.items[0].delivery_outcome, DeliveryOutcome::Silenced);
    assert_eq!(chat.items[0].actions[0].key, "reply");

    let calendar = projection
        .groups
        .iter()
        .find(|group| group.app_identity == "Calendar")
        .unwrap();
    assert_eq!(calendar.items[0].status.as_str(), "dismissed");
    assert!(calendar.items[0].actions.is_empty());
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
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        snapshot
            .active
            .iter()
            .find(|thread| thread.id == receipt.notification_id)
            .unwrap()
            .delivery_outcome,
        DeliveryOutcome::Shown
    );
    let subsequent = daemon
        .receive_notification(notification("Chat", "Subsequent message"))
        .await
        .unwrap();
    assert_eq!(subsequent.delivery_outcome, DeliveryOutcome::Silenced);

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
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        snapshot
            .history
            .iter()
            .find(|thread| thread.id == receipt.notification_id)
            .unwrap()
            .status
            .as_str(),
        "dismissed"
    );

    assert_eq!(
        control
            .execute(ControlCommand::DeleteNotificationHistory)
            .await
            .unwrap(),
        ControlReply::CommandApplied
    );
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.history.len(), 1);
    assert_eq!(snapshot.history[0].id, subsequent.notification_id);
}

#[tokio::test]
async fn notify_control_activates_notification_center_items() {
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());
    let mut events = daemon.subscribe_sender_events();

    let with_default = daemon
        .receive_notification(NotificationInput {
            actions: vec![NotificationAction::new("default", "Open")],
            ..notification("Mail", "Inbox")
        })
        .await
        .unwrap();

    assert_eq!(
        control
            .execute(ControlCommand::ActivateNotification {
                notification_id: with_default.notification_id,
            })
            .await
            .unwrap(),
        ControlReply::CommandApplied
    );
    let event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        event,
        SenderEvent::ActionInvoked {
            action_key,
            ..
        } if action_key == "default"
    ));

    let without_default = daemon
        .receive_notification(notification("Calendar", "Standup"))
        .await
        .unwrap();
    assert_eq!(
        control
            .execute(ControlCommand::ActivateNotification {
                notification_id: without_default.notification_id,
            })
            .await
            .unwrap(),
        ControlReply::CommandApplied
    );

    let snapshot = daemon.snapshot().await.unwrap();
    let dismissed = snapshot
        .history
        .iter()
        .find(|thread| thread.id == without_default.notification_id)
        .unwrap();
    assert_eq!(dismissed.status.as_str(), "dismissed");
    assert!(!daemon.sender_events().await.iter().any(|event| matches!(
        event,
        SenderEvent::ActionInvoked {
            notification_id,
            ..
        } if *notification_id == without_default.notification_id
    )));
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
    assert!(matches!(
        action_event,
        SenderEvent::ActionInvoked {
            notification_id,
            action_key,
        } if notification_id == receipt.notification_id && action_key == "open"
    ));

    let close_event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        close_event,
        SenderEvent::NotificationClosed {
            notification_id,
            ..
        } if notification_id == receipt.notification_id
    ));
}

#[tokio::test]
async fn action_invocation_emits_activation_token_before_action_and_close() {
    let provider = FakeActivationTokenProvider::new(["token-1"]);
    let (_temp, daemon) = open_daemon_with_activation_provider(provider.clone()).await;
    let mut events = daemon.subscribe_sender_events();

    let receipt = daemon
        .receive_notification(NotificationInput {
            desktop_entry: Some("chat.desktop".to_string()),
            actions: vec![NotificationAction::new("open", "Open")],
            ..notification("Chat", "Message")
        })
        .await
        .unwrap();

    daemon
        .invoke_action(receipt.notification_id, "open")
        .await
        .unwrap();

    let activation_event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        activation_event,
        SenderEvent::ActivationToken {
            notification_id,
            activation_token,
        } if notification_id == receipt.notification_id && activation_token == "token-1"
    ));

    let action_event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        action_event,
        SenderEvent::ActionInvoked {
            notification_id,
            action_key,
        } if notification_id == receipt.notification_id && action_key == "open"
    ));

    let closed_event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        closed_event,
        SenderEvent::NotificationClosed {
            notification_id,
            ..
        } if notification_id == receipt.notification_id
    ));
    assert_eq!(
        provider.requested_app_identities(),
        vec!["chat.desktop".to_string()]
    );
}

#[tokio::test]
async fn action_invocation_prefers_supplied_activation_token() {
    let provider = FakeActivationTokenProvider::new(["provider-token"]);
    let (_temp, daemon) = open_daemon_with_activation_provider(provider.clone()).await;
    let mut events = daemon.subscribe_sender_events();

    let receipt = daemon
        .receive_notification(NotificationInput {
            desktop_entry: Some("chat.desktop".to_string()),
            actions: vec![NotificationAction::new("open", "Open")],
            ..notification("Chat", "Message")
        })
        .await
        .unwrap();

    daemon
        .invoke_action_with_activation_token(
            receipt.notification_id,
            "open",
            Some("banner-token".to_string()),
        )
        .await
        .unwrap();

    let activation_event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        activation_event,
        SenderEvent::ActivationToken {
            notification_id,
            activation_token,
        } if notification_id == receipt.notification_id && activation_token == "banner-token"
    ));

    let action_event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        action_event,
        SenderEvent::ActionInvoked {
            notification_id,
            action_key,
        } if notification_id == receipt.notification_id && action_key == "open"
    ));
    assert!(provider.requested_app_identities().is_empty());
}

#[tokio::test]
async fn empty_supplied_activation_token_falls_back_to_provider() {
    let provider = FakeActivationTokenProvider::new(["provider-token"]);
    let (_temp, daemon) = open_daemon_with_activation_provider(provider.clone()).await;
    let mut events = daemon.subscribe_sender_events();

    let receipt = daemon
        .receive_notification(NotificationInput {
            desktop_entry: Some("chat.desktop".to_string()),
            actions: vec![NotificationAction::new("open", "Open")],
            ..notification("Chat", "Message")
        })
        .await
        .unwrap();

    daemon
        .invoke_action_with_activation_token(receipt.notification_id, "open", Some(String::new()))
        .await
        .unwrap();

    let activation_event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        activation_event,
        SenderEvent::ActivationToken {
            notification_id,
            activation_token,
        } if notification_id == receipt.notification_id && activation_token == "provider-token"
    ));
    assert_eq!(
        provider.requested_app_identities(),
        vec!["chat.desktop".to_string()]
    );
}

#[tokio::test]
async fn banner_activation_prefers_supplied_activation_token_for_default_action() {
    let provider = FakeActivationTokenProvider::new(["provider-token"]);
    let (_temp, daemon) = open_daemon_with_activation_provider(provider.clone()).await;
    let mut events = daemon.subscribe_sender_events();

    let receipt = daemon
        .receive_notification(NotificationInput {
            desktop_entry: Some("mail.desktop".to_string()),
            actions: vec![NotificationAction::new("default", "Open")],
            ..notification("Mail", "Inbox")
        })
        .await
        .unwrap();

    daemon
        .activate_banner_with_activation_token(
            receipt.notification_id,
            Some("banner-token".to_string()),
        )
        .await
        .unwrap();

    let activation_event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        activation_event,
        SenderEvent::ActivationToken {
            notification_id,
            activation_token,
        } if notification_id == receipt.notification_id && activation_token == "banner-token"
    ));

    let action_event = tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        action_event,
        SenderEvent::ActionInvoked {
            notification_id,
            action_key,
        } if notification_id == receipt.notification_id && action_key == "default"
    ));
    assert!(provider.requested_app_identities().is_empty());
}

#[tokio::test]
async fn empty_activation_token_is_skipped_without_blocking_action() {
    let provider = FakeActivationTokenProvider::new([""]);
    let (_temp, daemon) = open_daemon_with_activation_provider(provider).await;
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
    assert!(matches!(
        action_event,
        SenderEvent::ActionInvoked {
            notification_id,
            action_key,
        } if notification_id == receipt.notification_id && action_key == "open"
    ));
}

#[tokio::test]
async fn resident_notification_gets_fresh_activation_token_per_invocation() {
    let provider = FakeActivationTokenProvider::new(["token-1", "token-2"]);
    let (_temp, daemon) = open_daemon_with_activation_provider(provider).await;

    let receipt = daemon
        .receive_notification(NotificationInput {
            resident: true,
            actions: vec![NotificationAction::new("pause", "Pause")],
            ..notification("Music", "Playing")
        })
        .await
        .unwrap();

    daemon
        .invoke_action(receipt.notification_id, "pause")
        .await
        .unwrap();
    daemon
        .invoke_action(receipt.notification_id, "pause")
        .await
        .unwrap();

    assert_eq!(
        daemon
            .sender_events()
            .await
            .iter()
            .filter_map(|event| match event {
                SenderEvent::ActivationToken {
                    activation_token, ..
                } => Some(activation_token.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec!["token-1", "token-2"]
    );
}

#[tokio::test]
async fn activation_token_uses_current_replacement_revision_app_identity() {
    let provider = FakeActivationTokenProvider::new(["token-1"]);
    let (_temp, daemon) = open_daemon_with_activation_provider(provider.clone()).await;

    let original = daemon
        .receive_notification(NotificationInput {
            desktop_entry: Some("old.desktop".to_string()),
            actions: vec![NotificationAction::new("open", "Open")],
            ..notification("Old", "Original")
        })
        .await
        .unwrap();
    let replacement = daemon
        .receive_notification(NotificationInput {
            desktop_entry: Some("new.desktop".to_string()),
            replaces_id: Some(original.notification_id),
            actions: vec![NotificationAction::new("open", "Open")],
            ..notification("New", "Replacement")
        })
        .await
        .unwrap();

    daemon
        .invoke_action(replacement.notification_id, "open")
        .await
        .unwrap();

    assert_eq!(
        provider.requested_app_identities(),
        vec!["new.desktop".to_string()]
    );
}

#[tokio::test]
async fn activation_token_capability_is_provider_availability_at_open() {
    let available = FakeActivationTokenProvider::new(["token-1"]);
    let (_available_temp, available_daemon) = open_daemon_with_activation_provider(available).await;
    assert!(available_daemon.supports_activation_tokens());

    let unavailable = FakeActivationTokenProvider::unavailable();
    let (_unavailable_temp, unavailable_daemon) =
        open_daemon_with_activation_provider(unavailable).await;
    assert!(!unavailable_daemon.supports_activation_tokens());

    let default_daemon = daemon_support::open_headless_daemon().await;
    assert!(!default_daemon.supports_activation_tokens());
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
