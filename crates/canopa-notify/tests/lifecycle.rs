#[path = "support/daemon.rs"]
mod daemon_support;
#[path = "support/notifications.rs"]
mod notifications;

use canopa_notify::{
    AcceptedNotification, BannerLifecycleChange, Daemon, DaemonOptions, DeliveryOutcome,
    LifecycleDelivery, LifecycleEffect, NewNotificationThread, NotificationAction,
    NotificationBodyContent, NotificationHistory, NotificationInput, NotificationLifecycle,
    NotificationStatus, NotificationThread, NotificationTimeout, NotificationTimeoutChange,
    PersistPrecondition, SenderCloseReason, SenderEvent, Urgency,
};
use notifications::notification;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI64, Ordering},
};
use std::time::Duration;

#[derive(Clone, Default)]
struct MemoryHistory {
    threads: Arc<Mutex<Vec<NotificationThread>>>,
    preconditions: Arc<Mutex<Vec<PersistPrecondition>>>,
}

impl MemoryHistory {
    fn stored(&self) -> Vec<NotificationThread> {
        self.threads.lock().unwrap().clone()
    }

    fn preconditions(&self) -> Vec<PersistPrecondition> {
        self.preconditions.lock().unwrap().clone()
    }
}

impl NotificationHistory for MemoryHistory {
    async fn create_thread(
        &self,
        thread: NewNotificationThread,
    ) -> anyhow::Result<NotificationThread> {
        let mut threads = self.threads.lock().unwrap();
        let id = threads.len() as i64 + 1;
        let thread = NotificationThread {
            id,
            accepted_notification: thread.accepted_notification,
            status: thread.status,
            delivery_outcome: thread.delivery_outcome,
            rule_reference: thread.rule_reference,
            revision: 1,
            created_at: thread.created_at,
            updated_at: thread.updated_at,
            closed_at: None,
        };
        threads.push(thread.clone());
        Ok(thread)
    }

    async fn thread(&self, id: i64) -> anyhow::Result<NotificationThread> {
        self.threads
            .lock()
            .unwrap()
            .iter()
            .find(|thread| thread.id == id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing thread {id}"))
    }

    async fn persist_thread(
        &self,
        thread: NotificationThread,
        precondition: PersistPrecondition,
    ) -> anyhow::Result<NotificationThread> {
        self.preconditions.lock().unwrap().push(precondition);
        let mut threads = self.threads.lock().unwrap();
        let stored = threads
            .iter_mut()
            .find(|stored| stored.id == thread.id)
            .ok_or_else(|| anyhow::anyhow!("missing thread {}", thread.id))?;
        let expected_revision = match precondition {
            PersistPrecondition::ExpectedRevision(revision)
            | PersistPrecondition::ExpectedActiveRevision(revision) => revision,
        };
        if stored.revision != expected_revision {
            anyhow::bail!("persistence precondition failed for thread {}", thread.id);
        }
        if matches!(precondition, PersistPrecondition::ExpectedActiveRevision(_))
            && stored.status != NotificationStatus::Active
        {
            anyhow::bail!("persistence precondition failed for thread {}", thread.id);
        }
        *stored = thread;
        Ok(stored.clone())
    }

    async fn active_threads(&self) -> anyhow::Result<Vec<NotificationThread>> {
        Ok(self
            .threads
            .lock()
            .unwrap()
            .iter()
            .filter(|thread| thread.status == NotificationStatus::Active)
            .cloned()
            .collect())
    }
}

#[tokio::test]
async fn lifecycle_replacement_owns_thread_revision_shape() {
    let history = MemoryHistory::default();
    let clock = Arc::new(AtomicI64::new(100));
    let lifecycle = lifecycle_with_clock(history.clone(), clock.clone());

    lifecycle
        .receive(
            lifecycle_notification(None, "Original"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    clock.store(250, Ordering::SeqCst);
    let effects = lifecycle
        .receive(
            lifecycle_notification(Some(1), "Replacement"),
            LifecycleDelivery::silenced("user:Chat:*"),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    let stored = history.stored();
    let thread = &stored[0];
    assert_eq!(thread.accepted_notification.summary, "Replacement");
    assert_eq!(thread.status, NotificationStatus::Active);
    assert_eq!(thread.delivery_outcome, DeliveryOutcome::Silenced);
    assert_eq!(thread.rule_reference.as_deref(), Some("user:Chat:*"));
    assert_eq!(thread.revision, 2);
    assert_eq!(thread.created_at, 100);
    assert_eq!(thread.updated_at, 250);
    assert_eq!(thread.closed_at, None);
    assert_eq!(
        history.preconditions(),
        vec![PersistPrecondition::ExpectedRevision(1)]
    );
    assert!(effects.raw().iter().any(|effect| {
        matches!(
            effect,
            LifecycleEffect::ThreadRevised(revised) if revised.id == thread.id
        )
    }));
    assert!(effects.banner_changes().any(|change| {
        matches!(
            change,
            BannerLifecycleChange::Remove { notification_id } if notification_id == thread.id
        )
    }));
}

#[tokio::test]
async fn lifecycle_schedules_timeout_for_shown_notification() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history);

    let effects = lifecycle
        .receive(
            lifecycle_notification(None, "Timed"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Milliseconds(42),
        )
        .await
        .unwrap();

    assert!(effects.timeout_changes().any(|change| {
        matches!(
            change,
            NotificationTimeoutChange::Scheduled {
                notification_id: 1,
                revision: 1,
                duration,
            } if duration == Duration::from_millis(42)
        )
    }));
}

#[tokio::test]
async fn lifecycle_expires_timeout_when_revision_matches() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());

    lifecycle
        .receive(
            lifecycle_notification(None, "Timed"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    let effects = lifecycle
        .expire_timeout_if_current_revision(1, 1)
        .await
        .unwrap();

    let stored = history.stored();
    assert_eq!(stored[0].status, NotificationStatus::Expired);
    assert_eq!(
        history.preconditions(),
        vec![PersistPrecondition::ExpectedActiveRevision(1)]
    );
    assert!(effects.raw().iter().any(|effect| {
        matches!(
            effect,
            LifecycleEffect::ThreadExpired(thread) if thread.id == 1
        )
    }));
    assert!(effects.timeout_changes().any(|change| {
        matches!(
            change,
            NotificationTimeoutChange::Cancelled { notification_id: 1 }
        )
    }));
    assert!(effects.sender_events().any(|event| {
        matches!(
            event,
            SenderEvent::NotificationClosed {
                notification_id: 1,
                reason: SenderCloseReason::Expired
            }
        )
    }));
}

#[tokio::test]
async fn lifecycle_ignores_timeout_when_revision_is_stale() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());

    lifecycle
        .receive(
            lifecycle_notification(None, "Timed"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    let effects = lifecycle
        .expire_timeout_if_current_revision(1, 2)
        .await
        .unwrap();

    let stored = history.stored();
    assert_eq!(stored[0].status, NotificationStatus::Active);
    assert!(effects.is_empty());
    assert!(history.preconditions().is_empty());
}

#[tokio::test]
async fn lifecycle_ignores_timeout_when_thread_is_inactive() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());

    lifecycle
        .receive(
            lifecycle_notification(None, "Timed"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();
    lifecycle.dismiss(1).await.unwrap();
    let preconditions_before_timeout = history.preconditions();

    let effects = lifecycle
        .expire_timeout_if_current_revision(1, 1)
        .await
        .unwrap();

    let stored = history.stored();
    assert_eq!(stored[0].status, NotificationStatus::Dismissed);
    assert!(effects.is_empty());
    assert_eq!(history.preconditions(), preconditions_before_timeout);
}

#[tokio::test]
async fn replacement_with_ineligible_timeout_cancels_previous_timeout() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history);

    lifecycle
        .receive(
            lifecycle_notification(None, "Original"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Milliseconds(42),
        )
        .await
        .unwrap();

    let effects = lifecycle
        .receive(
            lifecycle_notification(Some(1), "Replacement"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    assert!(effects.timeout_changes().any(|change| {
        matches!(
            change,
            NotificationTimeoutChange::Cancelled { notification_id: 1 }
        )
    }));
    assert!(
        !effects
            .timeout_changes()
            .any(|change| matches!(change, NotificationTimeoutChange::Scheduled { .. }))
    );
}

#[tokio::test]
async fn lifecycle_transition_owns_status_timestamps_and_precondition() {
    let history = MemoryHistory::default();
    let clock = Arc::new(AtomicI64::new(100));
    let lifecycle = lifecycle_with_clock(history.clone(), clock.clone());
    let receipt = lifecycle
        .receive(
            lifecycle_notification(None, "Original"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap()
        .sender_receipt()
        .unwrap();

    clock.store(300, Ordering::SeqCst);
    let effects = lifecycle.dismiss(receipt.notification_id).await.unwrap();

    let stored = history.stored();
    let thread = &stored[0];
    assert_eq!(thread.status, NotificationStatus::Dismissed);
    assert_eq!(thread.updated_at, 300);
    assert_eq!(thread.closed_at, Some(300));
    assert_eq!(
        history.preconditions(),
        vec![PersistPrecondition::ExpectedActiveRevision(1)]
    );
    assert!(effects.raw().iter().any(|effect| {
        matches!(
            effect,
            LifecycleEffect::ThreadDismissed(dismissed) if dismissed.id == thread.id
        )
    }));
    assert!(effects.sender_events().any(|event| {
        matches!(
            event,
            SenderEvent::NotificationClosed {
                notification_id,
                reason: SenderCloseReason::Dismissed
            } if notification_id == thread.id
        )
    }));
}

#[tokio::test]
async fn lifecycle_marks_active_threads_stale_without_history_deciding_shape() {
    let history = MemoryHistory::default();
    let clock = Arc::new(AtomicI64::new(100));
    let lifecycle = lifecycle_with_clock(history.clone(), clock.clone());

    lifecycle
        .receive(
            lifecycle_notification(None, "One"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();
    lifecycle
        .receive(
            lifecycle_notification(None, "Two"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    clock.store(400, Ordering::SeqCst);
    let effects = lifecycle.mark_active_stale().await.unwrap();

    let stored = history.stored();
    assert_eq!(stored.len(), 2);
    assert!(stored.iter().all(|thread| {
        thread.status == NotificationStatus::Stale
            && thread.updated_at == 400
            && thread.closed_at == Some(400)
    }));
    assert_eq!(
        history.preconditions(),
        vec![
            PersistPrecondition::ExpectedActiveRevision(1),
            PersistPrecondition::ExpectedActiveRevision(1)
        ]
    );
    assert_eq!(
        effects
            .banner_changes()
            .filter(|change| matches!(change, BannerLifecycleChange::Remove { .. }))
            .count(),
        2
    );
}

#[tokio::test]
async fn lifecycle_creates_shown_notification_through_history_and_returns_receipt_effect() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());

    let effects = lifecycle
        .receive(
            AcceptedNotification {
                app_identity: "example-mail.desktop".to_string(),
                app_name: "Example Mail".to_string(),
                replaces_id: None,
                app_icon: None,
                summary: "Inbox".to_string(),
                body: "One unread message".to_string(),
                body_content: NotificationBodyContent::plain("One unread message"),
                raw_body: "<b>One unread message</b>".to_string(),
                urgency: Urgency::Normal,
                category: None,
                progress: None,
                image_reference: None,
                has_image_payload: false,
                actions: Vec::new(),
                resident: false,
            },
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    assert_eq!(history.stored().len(), 1);
    assert!(effects.raw().iter().any(|effect| matches!(
        effect,
        LifecycleEffect::ThreadCreated(thread)
            if thread.accepted_notification.app_identity == "example-mail.desktop"
                && thread.delivery_outcome == DeliveryOutcome::Shown
    )));
    assert_eq!(
        effects.sender_receipt().unwrap().delivery_outcome,
        DeliveryOutcome::Shown
    );
}

#[tokio::test]
async fn lifecycle_dropped_replacement_returns_receipt_without_revising_thread() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());

    lifecycle
        .receive(
            lifecycle_notification(None, "Original"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    let effects = lifecycle
        .receive(
            lifecycle_notification(Some(1), "Should not overwrite"),
            LifecycleDelivery::dropped("user:Chat:*"),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    let stored = history.stored();
    assert_eq!(stored[0].accepted_notification.summary, "Original");
    assert_eq!(stored[0].revision, 1);
    assert!(effects.raw().iter().any(|effect| matches!(
        effect,
        LifecycleEffect::NotificationDropped {
            replaces_id: Some(1)
        }
    )));
    assert_eq!(
        effects.sender_receipt().unwrap(),
        canopa_notify::NotificationReceipt {
            notification_id: 1,
            delivery_outcome: DeliveryOutcome::Dropped,
        }
    );
    assert!(effects.sender_events().next().is_none());
    assert!(effects.banner_changes().next().is_none());
    assert!(
        !effects
            .timeout_changes()
            .any(|change| matches!(change, NotificationTimeoutChange::Cancelled { .. }))
    );
}

#[tokio::test]
async fn lifecycle_new_drop_gets_ephemeral_receipt_and_undefined_terminal_event() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());

    let effects = lifecycle
        .receive(
            lifecycle_notification(None, "Dropped"),
            LifecycleDelivery::dropped("user:Chat:*"),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    let receipt = effects.sender_receipt().unwrap();
    assert_ne!(receipt.notification_id, 0);
    assert_eq!(receipt.delivery_outcome, DeliveryOutcome::Dropped);
    assert!(history.stored().is_empty());
    assert_eq!(
        effects.sender_events().collect::<Vec<_>>(),
        vec![SenderEvent::NotificationClosed {
            notification_id: receipt.notification_id,
            reason: SenderCloseReason::Undefined,
        }]
    );
}

#[tokio::test]
async fn lifecycle_missing_replacement_target_creates_new_thread() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());

    let effects = lifecycle
        .receive(
            lifecycle_notification(Some(99), "New thread"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    assert_eq!(history.stored().len(), 1);
    assert!(effects.raw().iter().any(|effect| matches!(
        effect,
        LifecycleEffect::ThreadCreated(thread)
            if thread.id == 1
                && thread.accepted_notification.summary == "New thread"
                && thread.accepted_notification.replaces_id == Some(99)
    )));
}

#[tokio::test]
async fn lifecycle_fixed_replacement_token_revises_active_thread_created_for_that_token() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());

    lifecycle
        .receive(
            lifecycle_notification(Some(9999), "0%"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    let effects = lifecycle
        .receive(
            lifecycle_notification(Some(9999), "5%"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    let stored = history.stored();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].id, 1);
    assert_eq!(stored[0].revision, 2);
    assert_eq!(stored[0].accepted_notification.summary, "5%");
    assert!(effects.raw().iter().any(|effect| matches!(
        effect,
        LifecycleEffect::ThreadRevised(thread) if thread.id == 1
    )));
}

fn lifecycle_notification(replaces_id: Option<i64>, summary: &str) -> AcceptedNotification {
    AcceptedNotification {
        app_identity: "Chat".to_string(),
        app_name: "Chat".to_string(),
        replaces_id,
        app_icon: None,
        summary: summary.to_string(),
        body: String::new(),
        body_content: NotificationBodyContent::default(),
        raw_body: String::new(),
        urgency: Urgency::Normal,
        category: None,
        progress: None,
        image_reference: None,
        has_image_payload: false,
        actions: Vec::new(),
        resident: false,
    }
}

fn lifecycle_with_clock(
    history: MemoryHistory,
    clock: Arc<AtomicI64>,
) -> NotificationLifecycle<MemoryHistory> {
    NotificationLifecycle::with_clock(history, move || clock.load(Ordering::SeqCst))
}

#[tokio::test]
async fn actions_dismiss_non_resident_notifications_but_resident_notifications_stay_active() {
    let daemon = daemon_support::open_headless_daemon().await;

    let non_resident = daemon
        .receive_notification(NotificationInput {
            actions: vec![NotificationAction::new("open", "Open")],
            ..notification("Calendar", "Standup")
        })
        .await
        .unwrap();

    daemon
        .invoke_action(non_resident.notification_id, "open")
        .await
        .unwrap();

    let non_resident_events = daemon.sender_events().await;
    assert_eq!(
        non_resident_events
            .iter()
            .map(|event| match event {
                SenderEvent::ActionInvoked {
                    notification_id,
                    action_key,
                } => (Some(action_key.as_str()), *notification_id, None),
                SenderEvent::NotificationClosed {
                    notification_id,
                    reason,
                } => (None, *notification_id, Some(reason.as_str())),
                SenderEvent::ActivationToken { .. } => unreachable!(),
            })
            .collect::<Vec<_>>(),
        vec![
            (Some("open"), non_resident.notification_id, None),
            (None, non_resident.notification_id, Some("dismissed"))
        ]
    );

    let resident = daemon
        .receive_notification(NotificationInput {
            resident: true,
            actions: vec![NotificationAction::new("pause", "Pause")],
            ..notification("Music", "Playing")
        })
        .await
        .unwrap();

    daemon
        .invoke_action(resident.notification_id, "pause")
        .await
        .unwrap();

    let snapshot = daemon.snapshot().await.unwrap();
    let dismissed = snapshot
        .history
        .iter()
        .find(|thread| thread.id == non_resident.notification_id)
        .unwrap();
    let still_active = snapshot
        .history
        .iter()
        .find(|thread| thread.id == resident.notification_id)
        .unwrap();

    assert_eq!(dismissed.status.as_str(), "dismissed");
    assert_eq!(still_active.status.as_str(), "active");

    let events = daemon.sender_events().await;
    assert!(events.iter().any(|event| {
        matches!(
            event,
            SenderEvent::ActionInvoked {
                notification_id,
                action_key,
            } if *notification_id == resident.notification_id && action_key == "pause"
        )
    }));
    assert!(!events.iter().any(|event| matches!(
        event,
        SenderEvent::NotificationClosed {
            notification_id,
            ..
        } if *notification_id == resident.notification_id
    )));
}

#[tokio::test]
async fn active_notifications_can_be_dismissed_expired_closed_and_marked_stale_after_restart() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();

    let dismissed = daemon
        .receive_notification(notification("Mail", "Dismiss me"))
        .await
        .unwrap();
    let expired = daemon
        .receive_notification(notification("Timer", "Expire me"))
        .await
        .unwrap();
    let closed = daemon
        .receive_notification(notification("Chat", "Close me"))
        .await
        .unwrap();
    let stale = daemon
        .receive_notification(notification("Build", "Still active"))
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
