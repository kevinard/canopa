//! Public callers may abandon their reply without cancelling committed effects.
use canopa_notify::{
    BannerPresenter, BannerView, Daemon, DaemonOptions, NotificationAction, NotificationInput,
    NotificationTimeout, NotifyControl, SenderCloseReason, SenderEvent, SenderEventPublication,
    SenderEventPublisher,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Default)]
struct Presenter(Arc<Mutex<Vec<BannerView>>>);
impl BannerPresenter for Presenter {
    fn replace_visible_banners(&self, banners: Vec<BannerView>) -> anyhow::Result<()> {
        *self.0.lock().unwrap() = banners;
        Ok(())
    }
}

#[derive(Clone)]
struct PausedPublisher {
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Semaphore>,
}
impl PausedPublisher {
    fn new() -> Self {
        Self {
            entered: Default::default(),
            release: Arc::new(tokio::sync::Semaphore::new(0)),
        }
    }
}
impl SenderEventPublisher for PausedPublisher {
    fn publish(&self, _event: SenderEvent) -> SenderEventPublication {
        let paused = self.clone();
        Box::pin(async move {
            paused.entered.notify_one();
            paused.release.acquire().await.unwrap().forget();
            Ok(())
        })
    }
}

#[tokio::test]
async fn cancelled_action_and_clear_waiters_do_not_abandon_committed_effects() {
    for clear in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let presenter = Presenter::default();
        let daemon = Daemon::open(DaemonOptions::graphical(
            directory.path().join("history.db"),
            presenter.clone(),
        ))
        .await
        .unwrap();
        let publisher = PausedPublisher::new();
        daemon
            .install_sender_event_publisher(publisher.clone())
            .unwrap();
        let receipt = daemon
            .receive_notification(NotificationInput {
                summary: "Committed notification".into(),
                expire_timeout: NotificationTimeout::Never,
                actions: vec![NotificationAction::new("reply", "Reply")],
                ..Default::default()
            })
            .await
            .unwrap();
        let mut changes = NotifyControl::new(daemon.clone()).subscribe_notification_state();
        assert!(changes.changed().await);
        let caller = tokio::spawn({
            let daemon = daemon.clone();
            async move {
                if clear {
                    daemon.clear_active_notifications().await
                } else {
                    daemon.invoke_action(receipt.notification_id, "reply").await
                }
            }
        });
        tokio::time::timeout(Duration::from_secs(1), publisher.entered.notified())
            .await
            .unwrap();
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        assert!(
            daemon.snapshot().await.unwrap().active.is_empty(),
            "the mutation already committed"
        );
        assert_eq!(
            presenter.0.lock().unwrap().len(),
            1,
            "publication deliberately blocks final presentation"
        );
        publisher.release.add_permits(2);
        assert!(
            tokio::time::timeout(Duration::from_secs(1), changes.changed())
                .await
                .expect("committed state change must finish after its waiter disappears")
        );
        assert!(presenter.0.lock().unwrap().is_empty());
        let mut expected = Vec::new();
        if !clear {
            expected.push(SenderEvent::ActionInvoked {
                notification_id: receipt.notification_id,
                action_key: "reply".into(),
            });
        }
        expected.push(SenderEvent::NotificationClosed {
            notification_id: receipt.notification_id,
            reason: SenderCloseReason::Dismissed,
        });
        assert_eq!(daemon.sender_events().await, expected);
        tokio::time::timeout(Duration::from_secs(1), daemon.shutdown())
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn cancelled_receipt_keeps_registration_and_publication_owned() {
    let directory = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(directory.path().join("history.db")))
        .await
        .unwrap();
    let publisher = PausedPublisher::new();
    daemon
        .install_sender_event_publisher(publisher.clone())
        .unwrap();
    let receipts = Arc::new(Mutex::new(Vec::new()));
    let mut changes = NotifyControl::new(daemon.clone()).subscribe_notification_state();
    assert!(changes.changed().await);
    let caller = tokio::spawn({
        let daemon = daemon.clone();
        let receipts = receipts.clone();
        async move {
            // Empty notifications receive an ID and immediate closure. Registration
            // must precede that closure even if the transport caller disconnects.
            daemon
                .receive_notification_with_receipt_hook(
                    NotificationInput::default(),
                    move |receipt| {
                        receipts.lock().unwrap().push(receipt.clone());
                    },
                )
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(1), publisher.entered.notified())
        .await
        .unwrap();
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    assert_eq!(receipts.lock().unwrap().len(), 1);
    publisher.release.add_permits(1);
    assert!(
        tokio::time::timeout(Duration::from_secs(1), changes.changed())
            .await
            .unwrap()
    );
    assert_eq!(
        daemon.sender_events().await,
        vec![SenderEvent::NotificationClosed {
            notification_id: receipts.lock().unwrap()[0].notification_id,
            reason: SenderCloseReason::Undefined,
        }]
    );
    daemon.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_drains_owned_completion_and_cancelled_admission_never_commits() {
    let directory = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(directory.path().join("history.db")))
        .await
        .unwrap();
    let publisher = PausedPublisher::new();
    daemon
        .install_sender_event_publisher(publisher.clone())
        .unwrap();
    let receipt = daemon
        .receive_notification(NotificationInput {
            summary: "Before shutdown".into(),
            expire_timeout: NotificationTimeout::Never,
            ..Default::default()
        })
        .await
        .unwrap();
    let caller = tokio::spawn({
        let daemon = daemon.clone();
        async move { daemon.close_notification(receipt.notification_id).await }
    });
    tokio::time::timeout(Duration::from_secs(1), publisher.entered.notified())
        .await
        .unwrap();
    caller.abort();
    assert!(caller.await.unwrap_err().is_cancelled());
    {
        let waiting = daemon.receive_notification_with_preparation(
            || panic!("cancelled receipt must never pass admission"),
            |_| {},
        );
        tokio::pin!(waiting);
        assert!(futures_util::poll!(waiting.as_mut()).is_pending());
    }
    let mut shutdown = tokio::spawn({
        let daemon = daemon.clone();
        async move { daemon.shutdown().await }
    });
    assert!(
        tokio::time::timeout(Duration::from_millis(20), &mut shutdown)
            .await
            .is_err()
    );
    publisher.release.add_permits(1);
    tokio::time::timeout(Duration::from_secs(1), shutdown)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        daemon.sender_events().await,
        vec![SenderEvent::NotificationClosed {
            notification_id: receipt.notification_id,
            reason: SenderCloseReason::Closed,
        }]
    );
    assert!(
        daemon
            .receive_notification(NotificationInput::default())
            .await
            .is_err()
    );
}
