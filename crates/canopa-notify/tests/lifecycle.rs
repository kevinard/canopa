use canopa_notify::InternalNotificationId;
const FIRST_ID: InternalNotificationId = InternalNotificationId::new(1).unwrap();
const SECOND_ID: InternalNotificationId = InternalNotificationId::new(2).unwrap();
#[path = "support/daemon.rs"]
mod daemon_support;
#[path = "support/notifications.rs"]
mod notifications;

use canopa_notify::{
    AcceptedNotification, BannerLifecycleChange, Daemon, DaemonOptions, DeliveryOutcome,
    LifecycleDelivery, LifecycleEffect, NewNotificationThread, NotificationAction,
    NotificationBodyContent, NotificationHistory, NotificationInput, NotificationLifecycle,
    NotificationStatus, NotificationThread, NotificationTimeout, NotificationTimeoutChange,
    NotificationTimeoutDefaults, PersistPrecondition, SenderCloseReason, SenderEvent, Urgency,
};
use notifications::notification;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI64, AtomicUsize, Ordering},
};
use std::time::Duration;

#[derive(Clone, Default)]
struct MemoryHistory {
    threads: Arc<Mutex<Vec<NotificationThread>>>,
    preconditions: Arc<Mutex<Vec<PersistPrecondition>>>,
    replace_after_read: Arc<Mutex<bool>>,
    point_reads: Arc<AtomicUsize>,
    scan_reads: Arc<AtomicUsize>,
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
        let id = InternalNotificationId::new(threads.len() as i64 + 1).unwrap();
        let thread = NotificationThread {
            id,
            requires_attention: thread.requires_attention,
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

    async fn thread(&self, id: InternalNotificationId) -> anyhow::Result<NotificationThread> {
        let mut threads = self.threads.lock().unwrap();
        let stored = threads
            .iter_mut()
            .find(|thread| thread.id == id)
            .ok_or_else(|| anyhow::anyhow!("missing thread {id}"))?;
        let snapshot = stored.clone();
        if std::mem::take(&mut *self.replace_after_read.lock().unwrap()) {
            stored.revision += 1;
            stored.accepted_notification.summary = "Concurrent replacement".to_string();
        }
        Ok(snapshot)
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

    async fn active_thread(
        &self,
        id: InternalNotificationId,
    ) -> anyhow::Result<Option<NotificationThread>> {
        self.point_reads.fetch_add(1, Ordering::Relaxed);
        Ok(self
            .threads
            .lock()
            .unwrap()
            .iter()
            .find(|thread| thread.id == id && thread.status == NotificationStatus::Active)
            .cloned())
    }

    async fn active_threads(&self) -> anyhow::Result<Vec<NotificationThread>> {
        self.scan_reads.fetch_add(1, Ordering::Relaxed);
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
async fn replacement_uses_one_point_read_regardless_of_other_live_threads() {
    for live_count in [1, 64, 1024] {
        let history = MemoryHistory::default();
        let lifecycle = NotificationLifecycle::new(history.clone());
        for _ in 0..live_count {
            lifecycle
                .receive(
                    lifecycle_notification(None, "Original"),
                    LifecycleDelivery::shown(),
                    NotificationTimeout::Never,
                )
                .await
                .unwrap();
        }
        lifecycle
            .receive(
                lifecycle_notification(Some(FIRST_ID), "Updated"),
                LifecycleDelivery::shown(),
                NotificationTimeout::Never,
            )
            .await
            .unwrap();
        assert_eq!(
            history.point_reads.load(Ordering::Relaxed),
            1,
            "{live_count} live threads"
        );
        assert_eq!(
            history.scan_reads.load(Ordering::Relaxed),
            0,
            "replacement must not enumerate unrelated threads"
        );
        assert_eq!(
            lifecycle
                .thread(FIRST_ID)
                .await
                .unwrap()
                .accepted_notification
                .summary,
            "Updated"
        );
    }
}

#[tokio::test]
async fn stale_timeout_or_action_cannot_transition_a_concurrent_replacement() {
    for action in [false, true] {
        let history = MemoryHistory::default();
        let lifecycle = NotificationLifecycle::new(history.clone());
        let mut notification = lifecycle_notification(None, "Original");
        notification.actions = vec![NotificationAction::new("open", "Open")];
        lifecycle
            .receive(
                notification,
                LifecycleDelivery::shown(),
                NotificationTimeout::Never,
            )
            .await
            .unwrap();
        // Storage advances revision after returning the snapshot the operation validates.
        *history.replace_after_read.lock().unwrap() = true;
        let result = if action {
            lifecycle.invoke_action(FIRST_ID, "open").await
        } else {
            lifecycle
                .expire_timeout_if_current_revision(FIRST_ID, 1)
                .await
        };
        assert!(
            result.is_err() || result.unwrap().is_empty(),
            "stale operation emitted effects"
        );
        let current = lifecycle.thread(FIRST_ID).await.unwrap();
        assert_eq!(current.revision, 2);
        assert_eq!(current.status, NotificationStatus::Active);
        assert_eq!(
            current.accepted_notification.summary,
            "Concurrent replacement"
        );
    }
}

#[tokio::test]
async fn activation_token_wait_does_not_commit_an_action_or_block_replacement() {
    #[derive(Clone)]
    struct WaitingToken(Arc<tokio::sync::Notify>);
    impl canopa_notify::ActivationTokenProvider for WaitingToken {
        fn available(&self) -> bool {
            true
        }
        fn activation_token(&self, _: String) -> canopa_notify::ActivationTokenFuture {
            let started = self.0.clone();
            Box::pin(async move {
                started.notify_one();
                std::future::pending().await
            })
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db"))
            .with_activation_token_provider(WaitingToken(started.clone())),
    )
    .await
    .unwrap();
    let receipt = daemon
        .receive_notification(NotificationInput {
            transient: true,
            actions: vec![NotificationAction::new("open", "Open")],
            ..notification("Mail", "Original")
        })
        .await
        .unwrap();
    let action_daemon = daemon.clone();
    let action = tokio::spawn(async move {
        action_daemon
            .invoke_action(receipt.notification_id, "open")
            .await
    });
    started.notified().await;
    assert_eq!(
        daemon.visible_banners().await.len(),
        1,
        "action committed before token acquisition completed"
    );
    let replacement = daemon
        .receive_notification(NotificationInput {
            transient: true,
            replaces_id: Some(receipt.notification_id),
            ..notification("Mail", "Replacement")
        })
        .await
        .unwrap();
    assert_eq!(replacement.notification_id, receipt.notification_id);
    assert!(
        action.await.unwrap().is_err(),
        "stale action succeeded after replacement"
    );
    assert!(daemon.sender_events().await.is_empty());
    assert_eq!(daemon.visible_banners().await[0].summary, "Replacement");
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
            lifecycle_notification(Some(FIRST_ID), "Replacement"),
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
        vec![PersistPrecondition::ExpectedActiveRevision(1)]
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
async fn lifecycle_requests_timeout_start_on_display_for_shown_notification() {
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
            NotificationTimeoutChange::StartsOnDisplay {
                notification_id: FIRST_ID,
                revision: 1,
                timeout: NotificationTimeout::Milliseconds(42),
                urgency: Urgency::Normal,
            }
        )
    }));
}

#[tokio::test]
async fn lifecycle_never_timeout_does_not_request_start_on_display() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history);

    let effects = lifecycle
        .receive(
            lifecycle_notification(None, "Untimed"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    assert!(effects.timeout_changes().next().is_none());
}

#[tokio::test]
async fn lifecycle_preserves_server_default_until_display() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::with_timeout_defaults(
        history,
        NotificationTimeoutDefaults::new(
            Some(Duration::from_millis(11)),
            Some(Duration::from_millis(22)),
            Some(Duration::from_millis(33)),
        ),
    );

    let effects = lifecycle
        .receive(
            lifecycle_notification(None, "Default timeout"),
            LifecycleDelivery::shown(),
            NotificationTimeout::ServerDefault,
        )
        .await
        .unwrap();

    assert!(effects.timeout_changes().any(|change| matches!(
        change,
        NotificationTimeoutChange::StartsOnDisplay {
            notification_id: FIRST_ID,
            revision: 1,
            timeout: NotificationTimeout::ServerDefault,
            urgency: Urgency::Normal,
        }
    )));
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
        .expire_timeout_if_current_revision(FIRST_ID, 1)
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
            LifecycleEffect::ThreadExpired(thread) if thread.id == FIRST_ID
        )
    }));
    assert!(effects.timeout_changes().any(|change| {
        matches!(
            change,
            NotificationTimeoutChange::CancelThroughRevision {
                notification_id: FIRST_ID,
                revision: 1,
            }
        )
    }));
    assert!(effects.sender_events().any(|event| {
        matches!(
            event,
            SenderEvent::NotificationClosed {
                notification_id: FIRST_ID,
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
        .expire_timeout_if_current_revision(FIRST_ID, 2)
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
    lifecycle.dismiss(FIRST_ID).await.unwrap();
    let preconditions_before_timeout = history.preconditions();

    let effects = lifecycle
        .expire_timeout_if_current_revision(FIRST_ID, 1)
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
            lifecycle_notification(Some(FIRST_ID), "Replacement"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    assert!(effects.timeout_changes().any(|change| {
        matches!(
            change,
            NotificationTimeoutChange::CancelThroughRevision {
                notification_id: FIRST_ID,
                revision: 1,
            }
        )
    }));
    assert!(
        !effects
            .timeout_changes()
            .any(|change| matches!(change, NotificationTimeoutChange::StartsOnDisplay { .. }))
    );
}

#[tokio::test]
async fn finite_replacement_cancels_previous_timeout_before_requesting_a_fresh_one() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history);

    lifecycle
        .receive(
            lifecycle_notification(None, "Original"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Milliseconds(100),
        )
        .await
        .unwrap();

    let effects = lifecycle
        .receive(
            lifecycle_notification(Some(FIRST_ID), "Replacement"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Milliseconds(200),
        )
        .await
        .unwrap();

    assert_eq!(
        effects.timeout_changes().collect::<Vec<_>>(),
        vec![
            NotificationTimeoutChange::CancelThroughRevision {
                notification_id: FIRST_ID,
                revision: 1,
            },
            NotificationTimeoutChange::StartsOnDisplay {
                notification_id: FIRST_ID,
                revision: 2,
                timeout: NotificationTimeout::Milliseconds(200),
                urgency: Urgency::Normal,
            },
        ]
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
                transient: false,
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
            lifecycle_notification(Some(FIRST_ID), "Should not overwrite"),
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
            replaces_id: Some(FIRST_ID)
        }
    )));
    assert_eq!(
        effects.sender_receipt().unwrap(),
        canopa_notify::NotificationReceipt {
            notification_id: FIRST_ID,
            delivery_outcome: DeliveryOutcome::Dropped,
        }
    );
    assert!(effects.sender_events().next().is_none());
    assert!(effects.banner_changes().next().is_none());
    assert!(!effects.timeout_changes().any(|change| matches!(
        change,
        NotificationTimeoutChange::CancelThroughRevision { .. }
    )));
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
    assert_ne!(receipt.notification_id.get(), 0);
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
async fn transient_thread_stays_runtime_only_across_replacement_and_action() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());
    let mut notification = lifecycle_notification(None, "Transient");
    notification.transient = true;
    notification.actions = vec![NotificationAction::new("open", "Open")];

    let created = lifecycle
        .receive(
            notification,
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();
    let notification_id = created.sender_receipt().unwrap().notification_id;
    assert!(notification_id.is_runtime_only());
    assert!(history.stored().is_empty());

    let mut replacement = lifecycle_notification(Some(notification_id), "Replacement");
    replacement.transient = false;
    replacement.actions = vec![NotificationAction::new("open", "Open")];
    let revised = lifecycle
        .receive(
            replacement,
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    assert_eq!(
        revised.sender_receipt().unwrap().notification_id,
        notification_id
    );
    let current = lifecycle.thread(notification_id).await.unwrap();
    assert_eq!(current.accepted_notification.summary, "Replacement");
    assert!(current.accepted_notification.transient);
    assert_eq!(current.revision, 2);
    assert!(history.stored().is_empty());

    let invoked = lifecycle
        .invoke_action(notification_id, "open")
        .await
        .unwrap();
    assert!(invoked.raw().iter().any(|effect| matches!(
        effect,
        LifecycleEffect::ActionInvoked { notification_id: id, action_key, .. }
            if *id == notification_id && action_key == "open"
    )));
    assert!(lifecycle.thread(notification_id).await.is_err());
    assert!(history.stored().is_empty());
}

#[tokio::test]
async fn retained_thread_cannot_be_weakened_by_transient_replacement_hint() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());
    let original = lifecycle
        .receive(
            lifecycle_notification(None, "Retained"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap()
        .sender_receipt()
        .unwrap();
    let mut replacement = lifecycle_notification(Some(original.notification_id), "Still retained");
    replacement.transient = true;

    lifecycle
        .receive(
            replacement,
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    let stored = history.stored();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].accepted_notification.summary, "Still retained");
    assert!(!stored[0].accepted_notification.transient);
    assert_eq!(stored[0].revision, 2);
}

#[tokio::test]
async fn lifecycle_missing_replacement_target_creates_new_thread() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());

    let effects = lifecycle
        .receive(
            lifecycle_notification(Some(InternalNotificationId::new(99).unwrap()), "New thread"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    assert_eq!(history.stored().len(), 1);
    assert!(effects.raw().iter().any(|effect| matches!(
        effect,
        LifecycleEffect::ThreadCreated(thread)
            if thread.id == FIRST_ID
                && thread.accepted_notification.summary == "New thread"
                && thread.accepted_notification.replaces_id == Some(InternalNotificationId::new(99).unwrap())
    )));
}

#[tokio::test]
async fn lifecycle_does_not_resolve_unrecognized_transport_aliases() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());

    lifecycle
        .receive(
            lifecycle_notification(Some(InternalNotificationId::new(9999).unwrap()), "0%"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    let effects = lifecycle
        .receive(
            lifecycle_notification(Some(InternalNotificationId::new(9999).unwrap()), "5%"),
            LifecycleDelivery::shown(),
            NotificationTimeout::Never,
        )
        .await
        .unwrap();

    let stored = history.stored();
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].id, FIRST_ID);
    assert_eq!(stored[0].revision, 1);
    assert_eq!(stored[0].accepted_notification.summary, "0%");
    assert_eq!(stored[1].accepted_notification.summary, "5%");
    assert!(effects.raw().iter().any(|effect| matches!(
        effect,
        LifecycleEffect::ThreadCreated(thread) if thread.id == SECOND_ID
    )));
}

fn lifecycle_notification(
    replaces_id: Option<InternalNotificationId>,
    summary: &str,
) -> AcceptedNotification {
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
        transient: false,
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

fn assert_status(
    snapshot: &canopa_notify::NotificationSnapshot,
    id: InternalNotificationId,
    status: &str,
) {
    let thread = snapshot
        .history
        .iter()
        .find(|thread| thread.id == id)
        .unwrap();
    assert_eq!(thread.status.as_str(), status);
}
