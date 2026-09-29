#[path = "support/daemon.rs"]
mod daemon_support;
#[path = "support/notifications.rs"]
mod notifications;

use canopa_notify::{
    ActivationTokenFuture, ActivationTokenProvider, AppPermissionControl, ControlCommand,
    ControlReply, Daemon, DaemonOptions, DeliveryOutcome, NotificationAction,
    NotificationBodyContent, NotificationBodyImage, NotificationBodySegment,
    NotificationCenterAction, NotificationCenterItem, NotificationInput, NotifyControl,
    SenderEvent,
};
use notifications::{notification, notification_with_body};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

async fn center_item(
    control: &NotifyControl,
    id: canopa_notify::InternalNotificationId,
) -> NotificationCenterItem {
    let ControlReply::NotificationCenterProjection(projection) = control
        .execute(ControlCommand::NotificationCenterProjection)
        .await
        .unwrap()
    else {
        panic!("expected center projection")
    };
    projection
        .groups
        .into_iter()
        .flat_map(|group| group.items)
        .find(|item| item.notification_id == id)
        .unwrap()
}

type LinkCalls = Arc<Mutex<Vec<(String, Option<String>)>>>;

#[derive(Clone, Default)]
struct RecordedLinks(LinkCalls);
impl canopa_notify::NotificationLinkOpener for RecordedLinks {
    fn available(&self) -> bool {
        true
    }
    fn open(
        &self,
        uri: canopa_notify::NotificationLink,
        token: Option<String>,
    ) -> canopa_notify::LinkOpeningFuture {
        let recorded = self.clone();
        Box::pin(async move {
            recorded
                .0
                .lock()
                .unwrap()
                .push((uri.as_str().into(), token));
            canopa_notify::LinkOpeningResult::Opened
        })
    }
}

#[tokio::test]
async fn queued_link_cannot_open_replaced_or_deleted_content() {
    #[derive(Clone, Default)]
    struct Gate {
        started: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }
    impl ActivationTokenProvider for Gate {
        fn available(&self) -> bool {
            true
        }
        fn activation_token(&self, identity: String) -> ActivationTokenFuture {
            assert!(identity.is_empty());
            let gate = self.clone();
            Box::pin(async move {
                gate.started.notify_one();
                gate.release.notified().await;
                Some("token".into())
            })
        }
    }
    for delete in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let gate = Gate::default();
        let links = RecordedLinks::default();
        let daemon = Daemon::open(
            DaemonOptions::headless(temp.path().join("history.db"))
                .with_link_opener(links.clone())
                .with_activation_token_provider(gate.clone()),
        )
        .await
        .unwrap();
        let control = NotifyControl::new(daemon.clone());
        let receipt = daemon
            .receive_notification(notification_with_body(
                "Mail",
                "Original",
                "<a href='https://example.org/old'>Read</a>",
            ))
            .await
            .unwrap();
        if delete {
            daemon
                .expire_notification(receipt.notification_id)
                .await
                .unwrap();
        }
        let captured = center_item(&control, receipt.notification_id).await;
        let mut invocation = Box::pin(control.execute(
            ControlCommand::InteractWithNotificationCenter {
                target: captured.target,
                action: NotificationCenterAction::OpenLink { segment_index: 0 },
            },
        ));
        tokio::select! {
            _ = gate.started.notified() => {},
            result = &mut invocation => panic!("link completed before token gate: {result:?}"),
        }
        tokio::time::timeout(Duration::from_secs(3), async {
            if delete {
                daemon.delete_notification_history().await.unwrap();
            } else {
                daemon
                    .receive_notification(NotificationInput {
                        replaces_id: Some(receipt.notification_id),
                        ..notification_with_body(
                            "Mail",
                            "Replacement",
                            "<a href='https://example.org/new'>Read</a>",
                        )
                    })
                    .await
                    .unwrap();
            }
        })
        .await
        .expect("token acquisition blocked a mutation");
        gate.release.notify_one();
        assert!(invocation.await.is_err());
        assert!(links.0.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn slow_link_chooser_is_bounded_and_does_not_hold_the_control_reply() {
    #[derive(Clone)]
    struct SlowOpener(Arc<tokio::sync::Semaphore>);
    impl canopa_notify::NotificationLinkOpener for SlowOpener {
        fn available(&self) -> bool {
            true
        }
        fn open(
            &self,
            _: canopa_notify::NotificationLink,
            _: Option<String>,
        ) -> canopa_notify::LinkOpeningFuture {
            let gate = self.0.clone();
            Box::pin(async move {
                let _ = gate.acquire().await;
                canopa_notify::LinkOpeningResult::Failed
            })
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db"))
            .with_link_opener(SlowOpener(gate.clone())),
    )
    .await
    .unwrap();
    let control = NotifyControl::new(daemon.clone());
    let receipt = daemon
        .receive_notification(notification_with_body(
            "Mail",
            "Message",
            "<a href='https://example.org/private?token=secret'>Read</a>",
        ))
        .await
        .unwrap();
    let target = center_item(&control, receipt.notification_id).await.target;
    let command = ControlCommand::InteractWithNotificationCenter {
        target,
        action: NotificationCenterAction::OpenLink { segment_index: 0 },
    };
    for _ in 0..8 {
        tokio::time::timeout(Duration::from_millis(250), control.execute(command.clone()))
            .await
            .expect("portal completion must not hold the control reply")
            .unwrap();
    }
    assert!(
        control.execute(command).await.is_err(),
        "a ninth pending opening must be rejected"
    );
    tokio::time::timeout(Duration::from_millis(250), daemon.set_do_not_disturb(true))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(daemon.link_opening_status().pending, 8);
    gate.close();
    wait_for_link_completion(&daemon).await;
    assert_eq!(
        daemon.link_opening_status().last_error.as_deref(),
        Some("notification link opening failed")
    );
    let ControlReply::Status(status) = control.execute(ControlCommand::Status).await.unwrap()
    else {
        panic!("status reply")
    };
    assert_eq!(status.link_opening, daemon.link_opening_status());
    assert!(!serde_json::to_string(&status).unwrap().contains("secret"));
    daemon.shutdown_link_opening().await.unwrap();
    assert!(!daemon.link_opening_status().available);
}

async fn wait_for_link_completion(daemon: &Daemon) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while daemon.link_opening_status().pending != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn center_links_open_the_captured_target_without_sender_actions_or_acknowledgement() {
    let temp = tempfile::tempdir().unwrap();
    let links = RecordedLinks::default();
    let tokens = FakeActivationTokenProvider::new(["browser-token", "history-token"]);
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db"))
            .with_link_opener(links.clone())
            .with_activation_token_provider(tokens.clone()),
    )
    .await
    .unwrap();
    let control = NotifyControl::new(daemon.clone());
    let receipt = daemon
        .receive_notification(NotificationInput {
            actions: vec![NotificationAction::new("default", "Open message")],
            ..notification_with_body(
                "Mail",
                "Message",
                "Read <a href='https://example.org/page'>the page</a>",
            )
        })
        .await
        .unwrap();
    let captured = center_item(&control, receipt.notification_id).await;
    let command = ControlCommand::InteractWithNotificationCenter {
        target: captured.target.clone(),
        action: NotificationCenterAction::OpenLink { segment_index: 1 },
    };
    control.execute(command.clone()).await.unwrap();
    wait_for_link_completion(&daemon).await;
    assert_eq!(
        *links.0.lock().unwrap(),
        vec![(
            "https://example.org/page".into(),
            Some("browser-token".into())
        )]
    );
    assert!(daemon.sender_events().await.is_empty());
    assert_eq!(
        center_item(&control, receipt.notification_id).await,
        captured
    );
    daemon
        .expire_notification(receipt.notification_id)
        .await
        .unwrap();
    let events = daemon.sender_events().await;
    control.execute(command).await.unwrap();
    wait_for_link_completion(&daemon).await;
    assert_eq!(links.0.lock().unwrap().len(), 2);
    assert_eq!(daemon.sender_events().await, events);
    assert!(
        center_item(&control, receipt.notification_id)
            .await
            .requires_attention
    );
    assert_eq!(
        tokens.requested_app_identities(),
        vec![String::new(), String::new()],
        "the selected browser is not the notification sender"
    );
}

#[tokio::test]
async fn retained_attention_can_be_acknowledged_after_expiry_without_sender_callbacks() {
    let temp = tempfile::tempdir().unwrap();
    let daemon =
        Daemon::open(DaemonOptions::headless(temp.path().join("history.db")).with_history_limit(0))
            .await
            .unwrap();
    let control = NotifyControl::new(daemon.clone());
    let receipt = daemon
        .receive_notification(notification("Mail", "Original"))
        .await
        .unwrap();
    let original = center_item(&control, receipt.notification_id).await;
    assert!(original.requires_attention);
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(receipt.notification_id),
            ..notification("Mail", "Replacement")
        })
        .await
        .unwrap();
    assert!(
        control
            .execute(ControlCommand::InteractWithNotificationCenter {
                target: original.target,
                action: NotificationCenterAction::Acknowledge,
            })
            .await
            .is_err()
    );
    daemon
        .expire_notification(receipt.notification_id)
        .await
        .unwrap();
    let expired = center_item(&control, receipt.notification_id).await;
    assert!(expired.requires_attention);
    let events = daemon.sender_events().await;
    control
        .execute(ControlCommand::InteractWithNotificationCenter {
            target: expired.target,
            action: NotificationCenterAction::Acknowledge,
        })
        .await
        .unwrap();
    assert_eq!(
        daemon.sender_events().await,
        events,
        "acknowledgement must not repeat closure or invoke stale actions"
    );
    assert!(
        daemon.snapshot().await.unwrap().history.is_empty(),
        "acknowledged archive may now be pruned"
    );
}

#[tokio::test]
async fn resident_actions_acknowledge_and_replacement_restores_attention() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let control = NotifyControl::new(daemon.clone());
    let input = NotificationInput {
        resident: true,
        actions: vec![NotificationAction::new("read", "Read")],
        ..notification("Mail", "Message")
    };
    let receipt = daemon.receive_notification(input.clone()).await.unwrap();
    assert!(
        center_item(&control, receipt.notification_id)
            .await
            .requires_attention
    );
    daemon
        .invoke_action(receipt.notification_id, "read")
        .await
        .unwrap();
    let read = center_item(&control, receipt.notification_id).await;
    assert!(!read.requires_attention);
    assert_eq!(read.status, canopa_notify::NotificationStatus::Active);
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(receipt.notification_id),
            ..input
        })
        .await
        .unwrap();
    assert!(
        center_item(&control, receipt.notification_id)
            .await
            .requires_attention
    );
    daemon
        .close_notification(receipt.notification_id)
        .await
        .unwrap();
    assert!(
        !center_item(&control, receipt.notification_id)
            .await
            .requires_attention
    );
    let transient = daemon
        .receive_notification(NotificationInput {
            transient: true,
            ..notification("Mail", "Transient")
        })
        .await
        .unwrap();
    assert!(
        !center_item(&control, transient.notification_id)
            .await
            .requires_attention
    );
    control
        .execute(ControlCommand::ChangeDoNotDisturb {
            enabled: Some(true),
        })
        .await
        .unwrap();
    let silenced = daemon
        .receive_notification(notification("Mail", "Silenced"))
        .await
        .unwrap();
    assert!(
        center_item(&control, silenced.notification_id)
            .await
            .requires_attention
    );
}

#[tokio::test]
async fn center_batches_merge_retained_and_transient_content_in_date_order() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let control = NotifyControl::new(daemon.clone());
    let mut expected = std::collections::BTreeSet::new();
    let mut chronological = Vec::new();
    for index in 0..19 {
        let receipt = daemon
            .receive_notification(NotificationInput {
                transient: index % 2 == 0,
                ..notification(
                    if index % 3 == 0 { "Chat" } else { "Mail" },
                    format!("Item {index}"),
                )
            })
            .await
            .unwrap();
        expected.insert(receipt.notification_id);
        chronological.push(receipt.notification_id);
        if index % 4 == 1 {
            daemon
                .dismiss_notification(receipt.notification_id)
                .await
                .unwrap();
        }
        // Runtime IDs count down; give each receipt a distinct millisecond so
        // this fixture checks dates independently of the stable ID tie-breaker.
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let mut after = None;
    let mut seen = std::collections::BTreeSet::new();
    chronological.reverse();
    for _ in 0..10 {
        let ControlReply::NotificationCenterBatch(batch) = control
            .execute(ControlCommand::NotificationCenterBatch { after, limit: 3 })
            .await
            .unwrap()
        else {
            panic!("expected bounded batch");
        };
        let items: Vec<_> = batch
            .projection
            .groups
            .into_iter()
            .flat_map(|group| group.items)
            .collect();
        assert!(items.len() <= 3);
        let expected_batch: std::collections::BTreeSet<_> = chronological
            .iter()
            .skip(seen.len())
            .take(3)
            .copied()
            .collect();
        assert_eq!(
            items
                .iter()
                .map(|item| item.notification_id)
                .collect::<std::collections::BTreeSet<_>>(),
            expected_batch,
            "batch boundaries follow date, regardless of app or attention"
        );
        for item in items {
            assert!(seen.insert(item.notification_id));
        }
        let Some(last) = batch.last else {
            break;
        };
        after = Some(last);
    }
    assert_eq!(seen, expected);
    for limit in [0, 17, 255] {
        assert!(
            control
                .execute(ControlCommand::NotificationCenterBatch { after: None, limit })
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn center_activation_rejects_replaced_and_terminal_targets_without_requesting_tokens() {
    let temp = tempfile::tempdir().unwrap();
    let tokens = FakeActivationTokenProvider::new(["new-token"; 4]);
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db"))
            .with_activation_token_provider(tokens.clone()),
    )
    .await
    .unwrap();
    let control = NotifyControl::new(daemon.clone());
    for transient in [false, true] {
        for had_default in [false, true] {
            let mut input = NotificationInput {
                transient,
                actions: vec![NotificationAction::new("same", "Named")],
                ..notification("Old app", "Original")
            };
            if had_default {
                input.actions.push(NotificationAction::new("default", ""));
            }
            let receipt = daemon.receive_notification(input.clone()).await.unwrap();
            let captured = center_item(&control, receipt.notification_id).await;
            input.replaces_id = Some(receipt.notification_id);
            input.app_name = "New app".into();
            input.summary = "Replacement".into();
            input.actions.retain(|action| action.key != "default");
            if !had_default {
                input.actions.push(NotificationAction::new("default", ""));
            }
            daemon.receive_notification(input).await.unwrap();
            let events = daemon.sender_events().await;
            let requests = tokens.requested_app_identities();
            for action in [
                NotificationCenterAction::Activate,
                NotificationCenterAction::InvokeAction {
                    action_key: "same".into(),
                },
            ] {
                assert!(
                    control
                        .execute(ControlCommand::InteractWithNotificationCenter {
                            target: captured.target.clone(),
                            action,
                        })
                        .await
                        .is_err()
                );
            }
            assert_eq!(tokens.requested_app_identities(), requests);
            assert_eq!(daemon.sender_events().await, events);
            let current = center_item(&control, receipt.notification_id).await;
            assert_ne!(current.target, captured.target);
            let activation = ControlCommand::InteractWithNotificationCenter {
                target: current.target,
                action: NotificationCenterAction::Activate,
            };
            // Exercise the same typed encoding consumed by socket and panel adapters.
            let activation: ControlCommand =
                serde_json::from_str(&serde_json::to_string(&activation).unwrap()).unwrap();
            control.execute(activation.clone()).await.unwrap();
            let committed = daemon.sender_events().await;
            let requests = tokens.requested_app_identities();
            assert!(control.execute(activation).await.is_err());
            assert_eq!(daemon.sender_events().await, committed);
            assert_eq!(tokens.requested_app_identities(), requests);
            assert_eq!(
                committed.len() - events.len(),
                if had_default { 1 } else { 3 }
            );
            if !had_default {
                assert_eq!(requests.last().unwrap(), "New app");
            }
        }
    }
}

#[tokio::test]
async fn center_token_wait_revalidates_after_replacement_or_close_without_blocking_mutation() {
    #[derive(Clone)]
    struct GatedToken {
        started: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }
    impl ActivationTokenProvider for GatedToken {
        fn available(&self) -> bool {
            true
        }
        fn activation_token(&self, _: String) -> ActivationTokenFuture {
            let gate = self.clone();
            Box::pin(async move {
                gate.started.notify_one();
                gate.release.notified().await;
                Some("obsolete-token".into())
            })
        }
    }
    for close in [false, true] {
        for action in [
            NotificationCenterAction::Activate,
            NotificationCenterAction::InvokeAction {
                action_key: "same".into(),
            },
        ] {
            let temp = tempfile::tempdir().unwrap();
            let gate = GatedToken {
                started: Arc::default(),
                release: Arc::default(),
            };
            let daemon = Daemon::open(
                DaemonOptions::headless(temp.path().join("history.db"))
                    .with_activation_token_provider(gate.clone()),
            )
            .await
            .unwrap();
            let control = NotifyControl::new(daemon.clone());
            let input = NotificationInput {
                transient: true,
                actions: vec![
                    NotificationAction::new("default", ""),
                    NotificationAction::new("same", "Named"),
                ],
                ..notification("Mail", "Original")
            };
            let receipt = daemon.receive_notification(input.clone()).await.unwrap();
            let captured = center_item(&control, receipt.notification_id).await;
            // Poll only to the token gate, then withhold further action polling until
            // mutation completes. The interleaving does not depend on a short sleep
            // or pausing the database's timers along with the activation deadline.
            let mut invocation = Box::pin(control.execute(
                ControlCommand::InteractWithNotificationCenter {
                    target: captured.target,
                    action,
                },
            ));
            assert!(futures_util::poll!(&mut invocation).is_pending());
            assert!(futures_util::FutureExt::now_or_never(gate.started.notified()).is_some());
            tokio::time::timeout(Duration::from_secs(3), async {
                if close {
                    daemon
                        .close_notification(receipt.notification_id)
                        .await
                        .unwrap();
                } else {
                    daemon
                        .receive_notification(NotificationInput {
                            replaces_id: Some(receipt.notification_id),
                            summary: "Replacement".into(),
                            ..input
                        })
                        .await
                        .unwrap();
                }
            })
            .await
            .expect("token acquisition must not hold the mutation coordinator");
            let events = daemon.sender_events().await;
            gate.release.notify_one();
            assert!(invocation.await.is_err());
            assert_eq!(daemon.sender_events().await, events);
        }
    }
}

#[tokio::test]
async fn control_ids_keep_signed_numeric_json_and_reject_zero() {
    let daemon = daemon_support::open_headless_daemon().await;
    let retained = daemon
        .receive_notification(notification("App", "Retained"))
        .await
        .unwrap();
    let transient = daemon
        .receive_notification(NotificationInput {
            transient: true,
            ..notification("App", "Transient")
        })
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&retained).unwrap(),
        serde_json::json!({
            "notification_id": 1, "delivery_outcome": "shown"
        })
    );
    assert_eq!(
        serde_json::to_value(&transient).unwrap(),
        serde_json::json!({
            "notification_id": -1, "delivery_outcome": "shown"
        })
    );
    let command: ControlCommand =
        serde_json::from_str(r#"{"command":"activate-notification","notification_id":-1}"#)
            .unwrap();
    assert_eq!(
        serde_json::to_value(&command).unwrap(),
        serde_json::json!({
            "command": "activate-notification", "notification_id": -1
        })
    );
    let control = NotifyControl::new(daemon.clone());
    assert_eq!(
        control.execute(command).await.unwrap(),
        ControlReply::CommandApplied
    );
    assert!(daemon.sender_events().await.iter().any(|event| matches!(event,
        SenderEvent::NotificationClosed { notification_id, .. } if *notification_id == transient.notification_id
    )));
    for invalid in [
        serde_json::json!(0),
        serde_json::json!("1"),
        serde_json::json!(1.5),
        serde_json::json!(u64::MAX),
    ] {
        assert!(
            serde_json::from_value::<ControlCommand>(serde_json::json!({
                "command": "activate-notification", "notification_id": invalid
            }))
            .is_err()
        );
    }
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.active.len(), 1);
    assert_eq!(snapshot.active[0].id, retained.notification_id);
}

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

    let ControlReply::Status(status) = reply else {
        panic!("expected status")
    };
    assert!(status.available);
    assert!(!status.do_not_disturb);
    assert_eq!(status.configuration.generation, 0);
    assert!(status.presentation_error.is_none());
    assert!(status.sender_publication_error.is_none());
}

#[tokio::test]
async fn rejected_actions_do_not_request_activation_tokens() {
    let provider = FakeActivationTokenProvider::new(["unused-token"]);
    let (_temp, daemon) = open_daemon_with_activation_provider(provider.clone()).await;
    let receipt = daemon
        .receive_notification(NotificationInput {
            actions: vec![NotificationAction::new("open", "Open")],
            ..notification("Mail", "Message")
        })
        .await
        .unwrap();
    assert!(
        daemon
            .invoke_action(receipt.notification_id, "missing")
            .await
            .is_err()
    );
    daemon
        .close_notification(receipt.notification_id)
        .await
        .unwrap();
    assert!(
        daemon
            .invoke_action(receipt.notification_id, "open")
            .await
            .is_err()
    );
    assert!(
        provider.requested_app_identities().is_empty(),
        "rejected actions requested compositor tokens"
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
async fn notification_center_exposes_all_transient_actions_without_retaining_history() {
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());
    let receipt = daemon
        .receive_notification(NotificationInput {
            transient: true,
            actions: vec![
                NotificationAction::new("default", ""),
                NotificationAction::new("one", "First"),
                NotificationAction::new("two", "Second"),
                NotificationAction::new("three", "Third"),
                NotificationAction::new("four", "Fourth"),
            ],
            ..notification("Preview", "Runtime only")
        })
        .await
        .unwrap();

    let ControlReply::NotificationCenterProjection(projection) = control
        .execute(ControlCommand::NotificationCenterProjection)
        .await
        .unwrap()
    else {
        panic!("expected center projection")
    };
    assert_eq!(projection.groups.len(), 1);
    assert_eq!(projection.groups[0].app_identity, "Preview");
    let item = &projection.groups[0].items[0];
    assert_eq!(item.notification_id, receipt.notification_id);
    assert_eq!(
        item.actions
            .iter()
            .map(|action| action.key.as_str())
            .collect::<Vec<_>>(),
        ["one", "two", "three", "four"]
    );
    assert!(daemon.snapshot().await.unwrap().history.is_empty());

    control
        .execute(ControlCommand::InvokeNotificationAction {
            notification_id: receipt.notification_id,
            action_key: "four".to_string(),
        })
        .await
        .unwrap();
    assert!(daemon.snapshot().await.unwrap().history.is_empty());
    let ControlReply::NotificationCenterProjection(projection) = control
        .execute(ControlCommand::NotificationCenterProjection)
        .await
        .unwrap()
    else {
        panic!("expected center projection")
    };
    assert!(projection.groups.is_empty());
}

#[tokio::test]
async fn center_subscription_tracks_transient_replacement_policy_and_cleanup() {
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());
    control
        .execute(ControlCommand::ChangeDoNotDisturb {
            enabled: Some(true),
        })
        .await
        .unwrap();
    control
        .execute(ControlCommand::SetCategoryPermission {
            app_identity: "Chat".to_string(),
            category: "im.received".to_string(),
            permission: AppPermissionControl::Silence,
        })
        .await
        .unwrap();
    let mut subscription = control.subscribe_notification_state();
    assert!(
        subscription
            .next_projection()
            .await
            .unwrap()
            .unwrap()
            .groups
            .is_empty()
    );
    let retained = daemon
        .receive_notification(notification("Chat", "Retained"))
        .await
        .unwrap();
    let transient = daemon
        .receive_notification(NotificationInput {
            transient: true,
            resident: true,
            category: Some("im.received".to_string()),
            actions: vec![NotificationAction::new("old", "Old action")],
            ..notification("Chat", "Runtime only")
        })
        .await
        .unwrap();
    let projection = tokio::time::timeout(Duration::from_secs(1), subscription.next_projection())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let group = &projection.groups[0];
    assert_eq!(group.items.len(), 2);
    assert_eq!(
        group.category_permissions[0].permission,
        AppPermissionControl::Silence
    );
    let item = group
        .items
        .iter()
        .find(|item| item.notification_id == transient.notification_id)
        .unwrap();
    assert!(item.transient);
    assert_eq!(item.delivery_outcome, DeliveryOutcome::Silenced);
    assert!(
        !group
            .items
            .iter()
            .find(|item| item.notification_id == retained.notification_id)
            .unwrap()
            .transient
    );
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(transient.notification_id),
            transient: false,
            resident: true,
            actions: vec![
                NotificationAction::new("default", ""),
                NotificationAction::new("new", "New action"),
            ],
            ..notification("Chat", "Replaced runtime content")
        })
        .await
        .unwrap();
    let projection = tokio::time::timeout(Duration::from_secs(1), subscription.next_projection())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let item = projection.groups[0]
        .items
        .iter()
        .find(|item| item.notification_id == transient.notification_id)
        .unwrap();
    assert!(
        item.transient,
        "replacement must not promote transient content into history"
    );
    assert_eq!(item.summary, "Replaced runtime content");
    assert_eq!(
        item.actions,
        vec![NotificationAction::new("new", "New action")]
    );
    assert!(
        control
            .execute(ControlCommand::InvokeNotificationAction {
                notification_id: transient.notification_id,
                action_key: "old".to_string(),
            })
            .await
            .is_err()
    );
    control
        .execute(ControlCommand::ActivateNotification {
            notification_id: transient.notification_id,
        })
        .await
        .unwrap();
    assert!(
        matches!(daemon.sender_events().await.last().unwrap(), SenderEvent::ActionInvoked {
        notification_id, action_key,
    } if *notification_id == transient.notification_id && action_key == "default")
    );
    control
        .execute(ControlCommand::DeleteNotificationHistory)
        .await
        .unwrap();
    let ControlReply::NotificationCenterProjection(projection) = control
        .execute(ControlCommand::NotificationCenterProjection)
        .await
        .unwrap()
    else {
        panic!("expected center projection")
    };
    assert_eq!(
        projection.groups[0].items.len(),
        2,
        "history deletion must not terminate live items"
    );
    control
        .execute(ControlCommand::ClearActiveNotifications)
        .await
        .unwrap();
    let projection = tokio::time::timeout(Duration::from_secs(1), subscription.next_projection())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(projection.groups[0].items.len(), 1);
    assert_eq!(
        projection.groups[0].items[0].notification_id,
        retained.notification_id
    );
    assert!(projection.groups[0].items[0].actions.is_empty());
    assert_eq!(daemon.snapshot().await.unwrap().history.len(), 1);
}

#[tokio::test]
async fn transient_center_order_survives_other_app_runtime_churn() {
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());
    for index in 0..32 {
        daemon
            .receive_notification(NotificationInput {
                transient: true,
                ..notification("Preview", format!("Preview {index}"))
            })
            .await
            .unwrap();
    }
    let ControlReply::NotificationCenterProjection(before) = control
        .execute(ControlCommand::NotificationCenterProjection)
        .await
        .unwrap()
    else {
        panic!("expected center projection")
    };
    assert_eq!(before.groups.len(), 1);
    assert_eq!(before.groups[0].items.len(), 32);
    let ids = before.groups[0]
        .items
        .iter()
        .map(|item| item.notification_id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), 32);
    assert!(before.groups[0].items.iter().all(|item| item.transient));
    let mut other_ids = Vec::new();
    for index in 0..64 {
        other_ids.push(
            daemon
                .receive_notification(NotificationInput {
                    transient: true,
                    ..notification("Other", format!("Other {index}"))
                })
                .await
                .unwrap()
                .notification_id,
        );
    }
    for id in other_ids {
        daemon.close_notification(id).await.unwrap();
    }
    let ControlReply::NotificationCenterProjection(after) = control
        .execute(ControlCommand::NotificationCenterProjection)
        .await
        .unwrap()
    else {
        panic!("expected center projection")
    };
    assert_eq!(
        after, before,
        "unchanged items must retain their content and ordering"
    );
    assert!(daemon.snapshot().await.unwrap().history.is_empty());
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
    assert!(mail.items[0].body_content.text_runs()[0].style.bold);
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
async fn notify_control_exposes_structured_body_content_with_plain_fallback() {
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());
    let expected_body_content = NotificationBodyContent::from_segments(vec![
        NotificationBodySegment::plain_text("Open "),
        NotificationBodySegment::body_image(NotificationBodyImage::local_path(
            "/tmp/canopa-chart.png",
            Some("chart".to_string()),
        )),
    ]);

    daemon
        .receive_notification(notification_with_body(
            "Reporter",
            "Report",
            r#"Open <img src="/tmp/canopa-chart.png" alt="chart"/>"#,
        ))
        .await
        .unwrap();

    let reply = control
        .execute(ControlCommand::ActiveNotifications)
        .await
        .unwrap();
    let ControlReply::ActiveNotifications(active) = reply else {
        panic!("expected active notifications reply");
    };
    assert_eq!(active[0].accepted_notification.body, "Open chart");
    assert_eq!(
        active[0].accepted_notification.body_content,
        expected_body_content
    );

    let reply = control
        .execute(ControlCommand::NotificationCenterProjection)
        .await
        .unwrap();
    let ControlReply::NotificationCenterProjection(projection) = reply else {
        panic!("expected notification center projection reply");
    };
    let item = &projection.groups[0].items[0];
    assert_eq!(item.body, "Open chart");
    assert_eq!(item.body_content, expected_body_content);
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
async fn notify_control_projects_and_applies_category_permissions_with_app_fallback() {
    let daemon = daemon_support::open_headless_daemon().await;
    let control = NotifyControl::new(daemon.clone());
    let mut existing = Vec::new();
    for category in ["transfer.complete", "im.received"] {
        existing.push(
            daemon
                .receive_notification(NotificationInput {
                    category: Some(category.to_string()),
                    ..notification("Chat", category)
                })
                .await
                .unwrap()
                .notification_id,
        );
    }
    control
        .execute(ControlCommand::SetAppPermission {
            app_identity: "Chat".to_string(),
            permission: AppPermissionControl::Silence,
        })
        .await
        .unwrap();
    control
        .execute(ControlCommand::SetCategoryPermission {
            app_identity: "Chat".to_string(),
            category: "im.received".to_string(),
            permission: AppPermissionControl::Allow,
        })
        .await
        .unwrap();

    let ControlReply::NotificationCenterProjection(projection) = control
        .execute(ControlCommand::NotificationCenterProjection)
        .await
        .unwrap()
    else {
        panic!("expected notification center projection reply");
    };
    let chat = projection
        .groups
        .iter()
        .find(|group| group.app_identity == "Chat")
        .unwrap();
    assert_eq!(chat.permission, AppPermissionControl::Silence);
    assert_eq!(
        chat.category_permissions
            .iter()
            .map(|control| (control.category.as_str(), control.permission))
            .collect::<Vec<_>>(),
        vec![
            ("im.received", AppPermissionControl::Allow),
            ("transfer.complete", AppPermissionControl::Silence),
        ]
    );
    let snapshot = daemon.snapshot().await.unwrap();
    assert!(snapshot.active.iter().all(|thread| {
        !existing.contains(&thread.id) || thread.delivery_outcome == DeliveryOutcome::Shown
    }));

    let allowed = daemon
        .receive_notification(NotificationInput {
            category: Some("im.received".to_string()),
            ..notification("Chat", "Allowed category")
        })
        .await
        .unwrap();
    let silenced = daemon
        .receive_notification(NotificationInput {
            category: Some("transfer.complete".to_string()),
            ..notification("Chat", "Fallback category")
        })
        .await
        .unwrap();
    assert_eq!(allowed.delivery_outcome, DeliveryOutcome::Shown);
    assert_eq!(silenced.delivery_outcome, DeliveryOutcome::Silenced);
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
