use canopa_notify::{
    Daemon, DaemonOptions, DeliveryOutcome, LifecycleDelivery, LifecycleEffect,
    LifecycleNotification, NewNotificationThread, NotificationAction, NotificationHistory,
    NotificationInput, NotificationLifecycle, NotificationStatus, NotificationThread,
    ThreadRevision, Urgency,
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct MemoryHistory {
    threads: Arc<Mutex<Vec<NotificationThread>>>,
}

impl MemoryHistory {
    fn stored(&self) -> Vec<NotificationThread> {
        self.threads.lock().unwrap().clone()
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
            app_identity: thread.app_identity,
            app_name: thread.app_name,
            summary: thread.summary,
            body: thread.body,
            raw_body: thread.raw_body,
            urgency: thread.urgency,
            status: thread.status,
            delivery_outcome: thread.delivery_outcome,
            rule_reference: thread.rule_reference,
            category: thread.category,
            progress: thread.progress,
            has_image_payload: false,
            actions: thread.actions,
            resident: thread.resident,
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

    async fn revise_thread(
        &self,
        id: i64,
        revision: ThreadRevision,
    ) -> anyhow::Result<NotificationThread> {
        let mut threads = self.threads.lock().unwrap();
        let thread = threads
            .iter_mut()
            .find(|thread| thread.id == id)
            .ok_or_else(|| anyhow::anyhow!("missing thread {id}"))?;
        thread.app_identity = revision.app_identity;
        thread.app_name = revision.app_name;
        thread.summary = revision.summary;
        thread.body = revision.body;
        thread.raw_body = revision.raw_body;
        thread.urgency = revision.urgency;
        thread.status = NotificationStatus::Active;
        thread.delivery_outcome = revision.delivery_outcome;
        thread.rule_reference = revision.rule_reference;
        thread.category = revision.category;
        thread.progress = revision.progress;
        thread.resident = revision.resident;
        thread.actions = revision.actions;
        thread.revision += 1;
        thread.updated_at = revision.updated_at;
        thread.closed_at = None;
        Ok(thread.clone())
    }

    async fn transition_thread(
        &self,
        id: i64,
        status: NotificationStatus,
        updated_at: i64,
    ) -> anyhow::Result<NotificationThread> {
        let mut threads = self.threads.lock().unwrap();
        let thread = threads
            .iter_mut()
            .find(|thread| thread.id == id)
            .ok_or_else(|| anyhow::anyhow!("missing thread {id}"))?;
        thread.status = status;
        thread.updated_at = updated_at;
        thread.closed_at = Some(updated_at);
        Ok(thread.clone())
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

    async fn update_delivery(
        &self,
        id: i64,
        delivery: LifecycleDelivery,
    ) -> anyhow::Result<NotificationThread> {
        let mut threads = self.threads.lock().unwrap();
        let thread = threads
            .iter_mut()
            .find(|thread| thread.id == id)
            .ok_or_else(|| anyhow::anyhow!("missing thread {id}"))?;
        thread.delivery_outcome = delivery.outcome;
        thread.rule_reference = delivery.rule_reference;
        Ok(thread.clone())
    }

    async fn mark_active_threads_stale(
        &self,
        updated_at: i64,
    ) -> anyhow::Result<Vec<NotificationThread>> {
        let mut stale = Vec::new();
        let mut threads = self.threads.lock().unwrap();
        for thread in threads
            .iter_mut()
            .filter(|thread| thread.status == NotificationStatus::Active)
        {
            thread.status = NotificationStatus::Stale;
            thread.updated_at = updated_at;
            thread.closed_at = Some(updated_at);
            stale.push(thread.clone());
        }
        Ok(stale)
    }
}

#[tokio::test]
async fn lifecycle_creates_shown_notification_through_history_and_returns_receipt_effect() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());

    let effects = lifecycle
        .receive(
            LifecycleNotification {
                app_identity: "example-mail.desktop".to_string(),
                app_name: "Example Mail".to_string(),
                replaces_id: None,
                summary: "Inbox".to_string(),
                body: "One unread message".to_string(),
                raw_body: "<b>One unread message</b>".to_string(),
                urgency: Urgency::Normal,
                category: None,
                progress: None,
                actions: Vec::new(),
                resident: false,
            },
            LifecycleDelivery::shown(),
        )
        .await
        .unwrap();

    assert_eq!(history.stored().len(), 1);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        LifecycleEffect::ThreadCreated(thread)
            if thread.app_identity == "example-mail.desktop"
                && thread.delivery_outcome == DeliveryOutcome::Shown
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        LifecycleEffect::SenderReceipt(receipt)
            if receipt.notification_id == 1
                && receipt.delivery_outcome == DeliveryOutcome::Shown
    )));
}

#[tokio::test]
async fn lifecycle_dropped_replacement_returns_receipt_without_revising_thread() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());

    lifecycle
        .receive(
            lifecycle_notification(None, "Original"),
            LifecycleDelivery::shown(),
        )
        .await
        .unwrap();

    let effects = lifecycle
        .receive(
            lifecycle_notification(Some(1), "Should not overwrite"),
            LifecycleDelivery::dropped("user:Chat:*"),
        )
        .await
        .unwrap();

    let stored = history.stored();
    assert_eq!(stored[0].summary, "Original");
    assert_eq!(stored[0].revision, 1);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        LifecycleEffect::NotificationDropped {
            replaces_id: Some(1)
        }
    )));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        LifecycleEffect::SenderReceipt(receipt)
            if receipt.notification_id == 0
                && receipt.delivery_outcome == DeliveryOutcome::Dropped
    )));
}

#[tokio::test]
async fn lifecycle_missing_replacement_target_creates_new_thread() {
    let history = MemoryHistory::default();
    let lifecycle = NotificationLifecycle::new(history.clone());

    let effects = lifecycle
        .receive(
            lifecycle_notification(Some(99), "New thread"),
            LifecycleDelivery::shown(),
        )
        .await
        .unwrap();

    assert_eq!(history.stored().len(), 1);
    assert!(effects.iter().any(|effect| matches!(
        effect,
        LifecycleEffect::ThreadCreated(thread)
            if thread.id == 1 && thread.summary == "New thread"
    )));
}

fn lifecycle_notification(replaces_id: Option<i64>, summary: &str) -> LifecycleNotification {
    LifecycleNotification {
        app_identity: "Chat".to_string(),
        app_name: "Chat".to_string(),
        replaces_id,
        summary: summary.to_string(),
        body: String::new(),
        raw_body: String::new(),
        urgency: Urgency::Normal,
        category: None,
        progress: None,
        actions: Vec::new(),
        resident: false,
    }
}

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
