use canopa_notify::{
    BannerPresentation, BannerPresentationAcknowledgement, BannerPresenter, BannerView,
    ControlCommand, ControlReply, Daemon, DaemonOptions, NotificationInput, NotificationTimeout,
    NotifyControl, SenderCloseReason, SenderEvent, SenderEventPublication, SenderEventPublisher,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Default)]
struct DeferredPresenter {
    acknowledgements: Arc<Mutex<Vec<BannerPresentationAcknowledgement>>>,
}

impl DeferredPresenter {
    fn take_latest(&self) -> BannerPresentationAcknowledgement {
        self.acknowledgements.lock().unwrap().pop().unwrap()
    }
}

impl BannerPresenter for DeferredPresenter {
    fn replace_visible_banners(&self, _banners: Vec<BannerView>) -> anyhow::Result<()> {
        Ok(())
    }

    fn replace_banner_presentation_with_acknowledgement(
        &self,
        presentation: BannerPresentation,
        acknowledgement: BannerPresentationAcknowledgement,
    ) -> anyhow::Result<()> {
        if !presentation.banners.is_empty() {
            self.acknowledgements.lock().unwrap().push(acknowledgement);
        }
        Ok(())
    }
}

fn notification(timeout: NotificationTimeout) -> NotificationInput {
    NotificationInput {
        summary: "Timer".into(),
        expire_timeout: timeout,
        ..Default::default()
    }
}

async fn timeout_records(daemon: &Daemon) -> usize {
    let ControlReply::Status(status) = NotifyControl::new(daemon.clone())
        .execute(ControlCommand::Status)
        .await
        .unwrap()
    else {
        panic!("expected control status");
    };
    status.timeout_records
}

#[tokio::test]
async fn completed_retained_and_transient_churn_releases_timeout_bookkeeping() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    assert_eq!(timeout_records(&daemon).await, 0);
    for transient in [false, true] {
        for dnd in [false, true] {
            daemon.set_do_not_disturb(dnd).await.unwrap();
            for timeout in [
                NotificationTimeout::Never,
                NotificationTimeout::Milliseconds(60_000),
            ] {
                for _ in 0..64 {
                    let receipt = daemon
                        .receive_notification(NotificationInput {
                            summary: "Timer churn".into(),
                            transient,
                            expire_timeout: timeout,
                            ..Default::default()
                        })
                        .await
                        .unwrap();
                    daemon
                        .close_notification(receipt.notification_id)
                        .await
                        .unwrap();
                }
                assert_eq!(
                    timeout_records(&daemon).await,
                    0,
                    "completed notifications retained timeout records"
                );
            }
        }
    }
}

#[tokio::test]
async fn reclaimed_timers_cannot_be_recreated_by_late_display_acknowledgements() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = DeferredPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();
    let mut delayed = Vec::new();
    for transient in [false, true] {
        for _ in 0..64 {
            let receipt = daemon
                .receive_notification(NotificationInput {
                    transient,
                    ..notification(NotificationTimeout::Milliseconds(60_000))
                })
                .await
                .unwrap();
            assert_eq!(timeout_records(&daemon).await, 1);
            delayed.push(presenter.take_latest());
            daemon
                .close_notification(receipt.notification_id)
                .await
                .unwrap();
            assert_eq!(timeout_records(&daemon).await, 0);
        }
    }
    let events = daemon.sender_events().await;
    for acknowledgement in delayed.into_iter().rev() {
        acknowledgement.acknowledge();
        // Scheduling is synchronous with acknowledgement, so no sleep is needed
        // to detect an obsolete task being recreated and left running.
        assert_eq!(timeout_records(&daemon).await, 0);
    }
    assert_eq!(daemon.sender_events().await, events);
}

#[tokio::test]
async fn old_frames_cannot_start_replacements_or_restart_completed_timeouts() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = DeferredPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();
    let receipt = daemon
        .receive_notification(notification(NotificationTimeout::Milliseconds(20)))
        .await
        .unwrap();
    let old_frame = presenter.take_latest();
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(receipt.notification_id),
            summary: "Replacement".into(),
            ..notification(NotificationTimeout::Milliseconds(20))
        })
        .await
        .unwrap();
    let current_frame = presenter.take_latest();
    let other = daemon
        .receive_notification(notification(NotificationTimeout::Never))
        .await
        .unwrap();
    // This independently captured frame also contains the replacement revision.
    let duplicate_frame = presenter.take_latest();
    daemon
        .close_notification(other.notification_id)
        .await
        .unwrap();
    old_frame.acknowledge();
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(
        daemon.snapshot().await.unwrap().active[0].id,
        receipt.notification_id
    );
    assert_eq!(timeout_records(&daemon).await, 1);

    let mut events = daemon.subscribe_sender_events();
    current_frame.acknowledge();
    let event = tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(event, SenderEvent::NotificationClosed { notification_id, reason: SenderCloseReason::Expired } if notification_id == receipt.notification_id)
    );
    assert_eq!(timeout_records(&daemon).await, 0);
    let before = daemon.sender_events().await;
    duplicate_frame.acknowledge();
    assert_eq!(timeout_records(&daemon).await, 0);
    assert_eq!(daemon.sender_events().await, before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn display_acknowledgement_and_close_share_one_timer_lifetime() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = DeferredPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();
    for iteration in 0..128 {
        let receipt = daemon
            .receive_notification(NotificationInput {
                transient: true,
                ..notification(NotificationTimeout::Milliseconds(60_000))
            })
            .await
            .unwrap();
        let acknowledgement = presenter.take_latest();
        match iteration % 3 {
            0 => {
                acknowledgement.acknowledge();
                assert_eq!(timeout_records(&daemon).await, 1);
                daemon
                    .close_notification(receipt.notification_id)
                    .await
                    .unwrap();
            }
            1 => {
                daemon
                    .close_notification(receipt.notification_id)
                    .await
                    .unwrap();
                acknowledgement.acknowledge();
            }
            _ => {
                let barrier = Arc::new(tokio::sync::Barrier::new(2));
                let worker_barrier = barrier.clone();
                let display = tokio::spawn(async move {
                    worker_barrier.wait().await;
                    acknowledgement.acknowledge();
                });
                barrier.wait().await;
                daemon
                    .close_notification(receipt.notification_id)
                    .await
                    .unwrap();
                display.await.unwrap();
            }
        }
        assert_eq!(timeout_records(&daemon).await, 0);
    }
}

#[tokio::test]
async fn failed_writes_keep_timers_and_partial_clear_cancels_only_committed_threads() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let presenter = DeferredPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        database.clone(),
        presenter.clone(),
    ))
    .await
    .unwrap();
    let blocked = daemon
        .receive_notification(NotificationInput {
            summary: "Fails".into(),
            ..notification(NotificationTimeout::Milliseconds(60_000))
        })
        .await
        .unwrap();
    daemon
        .receive_notification(notification(NotificationTimeout::Milliseconds(60_000)))
        .await
        .unwrap();
    presenter.take_latest().acknowledge();
    assert_eq!(timeout_records(&daemon).await, 2);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&database))
        .await
        .unwrap();
    // Fixture-only storage failure injection; observations use daemon/control interfaces.
    sqlx::query("CREATE TRIGGER fail_timer_transition BEFORE UPDATE ON notification_threads WHEN OLD.summary = 'Fails' BEGIN SELECT RAISE(ABORT, 'injected transition failure'); END")
        .execute(&pool).await.unwrap();
    assert!(
        daemon
            .receive_notification(NotificationInput {
                replaces_id: Some(blocked.notification_id),
                ..notification(NotificationTimeout::Never)
            })
            .await
            .is_err()
    );
    assert!(
        daemon
            .close_notification(blocked.notification_id)
            .await
            .is_err()
    );
    assert_eq!(timeout_records(&daemon).await, 2);
    assert!(daemon.clear_active_notifications().await.is_err());
    assert_eq!(
        daemon.snapshot().await.unwrap().active[0].id,
        blocked.notification_id
    );
    assert_eq!(timeout_records(&daemon).await, 1);
    presenter.take_latest().acknowledge();
    assert_eq!(timeout_records(&daemon).await, 1);
    sqlx::query("DROP TRIGGER fail_timer_transition")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    daemon
        .close_notification(blocked.notification_id)
        .await
        .unwrap();
    assert_eq!(timeout_records(&daemon).await, 0);
}

#[derive(Clone, Default)]
struct PausedPublisher {
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    completed: Arc<tokio::sync::Notify>,
}

struct FailingExpiry {
    _directory: tempfile::TempDir,
    daemon: Daemon,
    presenter: DeferredPresenter,
    pool: sqlx::SqlitePool,
    receipt: canopa_notify::NotificationReceipt,
}

impl FailingExpiry {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("history.db");
        let presenter = DeferredPresenter::default();
        let daemon = Daemon::open(DaemonOptions::graphical(
            database.clone(),
            presenter.clone(),
        ))
        .await
        .unwrap();
        let receipt = daemon
            .receive_notification(notification(NotificationTimeout::Milliseconds(20)))
            .await
            .unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&database))
            .await
            .unwrap();
        sqlx::query("CREATE TRIGGER fail_expiry BEFORE UPDATE ON notification_threads WHEN NEW.status = 'expired' BEGIN SELECT RAISE(ABORT, 'injected expiry failure'); END")
            .execute(&pool).await.unwrap();
        Self {
            _directory: directory,
            daemon,
            presenter,
            pool,
            receipt,
        }
    }

    async fn recover_storage(&self) {
        sqlx::query("DROP TRIGGER fail_expiry")
            .execute(&self.pool)
            .await
            .unwrap();
    }

    async fn wait_for_failure(&self, terminal: bool) -> canopa_notify::ControlStatus {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let ControlReply::Status(status) = NotifyControl::new(self.daemon.clone())
                    .execute(ControlCommand::Status)
                    .await
                    .unwrap()
                else {
                    unreachable!()
                };
                if if terminal {
                    status.timeout_failed == 1
                } else {
                    status.timeout_retrying == 1
                } {
                    return *status;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("expiry failure must be observable")
    }
}

#[tokio::test]
async fn failed_firing_expiry_recovers_when_storage_becomes_writable() {
    let fixture = FailingExpiry::new().await;
    let daemon = &fixture.daemon;
    let mut events = daemon.subscribe_sender_events();
    fixture.presenter.take_latest().acknowledge();
    fixture.wait_for_failure(false).await;
    assert_eq!(
        timeout_records(daemon).await,
        1,
        "failed expiry must retain recoverable work"
    );
    assert_eq!(daemon.visible_banners().await.len(), 1);
    fixture.recover_storage().await;
    let event = tokio::time::timeout(Duration::from_secs(3), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(event, SenderEvent::NotificationClosed { notification_id, reason: SenderCloseReason::Expired } if notification_id == fixture.receipt.notification_id)
    );
    assert!(daemon.snapshot().await.unwrap().active.is_empty());
    assert_eq!(timeout_records(daemon).await, 0);
    assert!(daemon.visible_banners().await.is_empty());
    fixture.pool.close().await;
    daemon.shutdown().await.unwrap();
}

#[tokio::test]
async fn replacement_and_shutdown_retire_failed_expiry_retries() {
    for shutdown in [false, true] {
        let fixture = FailingExpiry::new().await;
        let daemon = &fixture.daemon;
        fixture.presenter.take_latest().acknowledge();
        fixture.wait_for_failure(false).await;
        if shutdown {
            daemon.shutdown().await.unwrap();
        } else {
            daemon
                .receive_notification(NotificationInput {
                    replaces_id: Some(fixture.receipt.notification_id),
                    summary: "Replacement".into(),
                    ..notification(NotificationTimeout::Never)
                })
                .await
                .unwrap();
            fixture.presenter.take_latest().acknowledge();
        }
        assert_eq!(timeout_records(daemon).await, 0);
        fixture.recover_storage().await;
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert_eq!(daemon.snapshot().await.unwrap().active.len(), 1);
        assert!(
            daemon.sender_events().await.is_empty(),
            "obsolete retry expired a live notification"
        );
        assert_eq!(timeout_records(daemon).await, 0);
        fixture.pool.close().await;
        if !shutdown {
            daemon.shutdown().await.unwrap();
        }
    }
}

#[tokio::test]
async fn exhausted_expiry_retries_remain_reported_until_explicit_completion() {
    let fixture = FailingExpiry::new().await;
    let daemon = &fixture.daemon;
    fixture.presenter.take_latest().acknowledge();
    let status = fixture.wait_for_failure(true).await;
    assert_eq!(
        (
            status.timeout_records,
            status.timeout_retrying,
            status.timeout_failed
        ),
        (1, 0, 1)
    );
    assert_eq!(daemon.snapshot().await.unwrap().active.len(), 1);
    fixture.recover_storage().await;
    daemon
        .expire_notification(fixture.receipt.notification_id)
        .await
        .unwrap();
    assert_eq!(timeout_records(daemon).await, 0);
    assert!(daemon.visible_banners().await.is_empty());
    fixture.pool.close().await;
    daemon.shutdown().await.unwrap();
}

impl SenderEventPublisher for PausedPublisher {
    fn publish(&self, _event: SenderEvent) -> SenderEventPublication {
        let publisher = self.clone();
        Box::pin(async move {
            publisher.entered.notify_one();
            publisher.release.notified().await;
            publisher.completed.notify_one();
            Ok(())
        })
    }
}

#[tokio::test]
async fn firing_timer_releases_its_record_without_aborting_committed_sender_publication() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = DeferredPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();
    let publisher = PausedPublisher::default();
    daemon
        .install_sender_event_publisher(publisher.clone())
        .unwrap();
    let receipt = daemon
        .receive_notification(notification(NotificationTimeout::Milliseconds(20)))
        .await
        .unwrap();
    presenter.take_latest().acknowledge();
    tokio::time::timeout(Duration::from_secs(2), publisher.entered.notified())
        .await
        .unwrap();
    assert_eq!(timeout_records(&daemon).await, 0);
    let snapshot = daemon.snapshot().await.unwrap();
    assert!(snapshot.active.is_empty());
    assert_eq!(snapshot.history[0].id, receipt.notification_id);
    assert_eq!(snapshot.history[0].status.as_str(), "expired");
    publisher.release.notify_one();
    tokio::time::timeout(Duration::from_secs(2), publisher.completed.notified())
        .await
        .unwrap();
    // Publication completed and released the mutation coordinator.
    tokio::time::timeout(
        Duration::from_secs(2),
        daemon.receive_notification(notification(NotificationTimeout::Never)),
    )
    .await
    .unwrap()
    .unwrap();
}

#[tokio::test]
async fn pending_display_records_stay_bounded_by_live_admission_and_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let input = NotificationInput {
        transient: true,
        ..notification(NotificationTimeout::Milliseconds(60_000))
    };
    for _ in 1..1024 {
        daemon.receive_notification(input.clone()).await.unwrap();
    }
    let first = daemon.receive_notification(input.clone()).await.unwrap();
    assert_eq!(timeout_records(&daemon).await, 1024);
    // A full live set retires its oldest thread, and that thread's record with it.
    daemon.receive_notification(input.clone()).await.unwrap();
    assert_eq!(timeout_records(&daemon).await, 1024);
    for _ in 0..64 {
        daemon
            .receive_notification(NotificationInput {
                replaces_id: Some(first.notification_id),
                ..input.clone()
            })
            .await
            .unwrap();
        assert_eq!(timeout_records(&daemon).await, 1024);
    }
    let dropped = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(first.notification_id),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(
        dropped.delivery_outcome,
        canopa_notify::DeliveryOutcome::Dropped
    );
    assert_eq!(timeout_records(&daemon).await, 1024);
    daemon.set_do_not_disturb(true).await.unwrap();
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(first.notification_id),
            ..input
        })
        .await
        .unwrap();
    // A silenced revision replaces the displayed one's record with its own; its
    // timeout begins at acceptance, except in Headless Mode, which expires nothing.
    assert_eq!(timeout_records(&daemon).await, 1024);
    daemon.clear_active_notifications().await.unwrap();
    assert_eq!(timeout_records(&daemon).await, 0);
}
