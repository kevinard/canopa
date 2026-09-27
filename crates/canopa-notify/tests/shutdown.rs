use canopa_notify::{
    BannerPresenter, BannerView, Daemon, DaemonOptions, NotificationAction, NotificationInput,
    NotificationStatus, SenderEvent, SenderEventPublication, SenderEventPublisher,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

#[derive(Clone, Default)]
struct Presentation(Arc<AtomicBool>);

impl BannerPresenter for Presentation {
    fn replace_visible_banners(&self, _: Vec<BannerView>) -> anyhow::Result<()> {
        Ok(())
    }
    fn shutdown(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

struct Transport {
    flushing: Arc<tokio::sync::Notify>,
    released: Arc<tokio::sync::Semaphore>,
}

impl SenderEventPublisher for Transport {
    fn publish(&self, _: SenderEvent) -> SenderEventPublication {
        Box::pin(async { Ok(()) })
    }
    fn flush(&self) -> SenderEventPublication {
        let flushing = self.flushing.clone();
        let released = self.released.clone();
        Box::pin(async move {
            flushing.notify_one();
            let _permit = released.acquire().await.unwrap();
            Ok(())
        })
    }
}

#[tokio::test]
async fn shutdown_retires_timers_and_mutations_then_waits_for_committed_publications() {
    let temp = tempfile::tempdir().unwrap();
    let presentation = Presentation::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presentation.clone(),
    ))
    .await
    .unwrap();
    let flushing = Arc::new(tokio::sync::Notify::new());
    let released = Arc::new(tokio::sync::Semaphore::new(0));
    daemon
        .install_sender_event_publisher(Transport {
            flushing: flushing.clone(),
            released: released.clone(),
        })
        .unwrap();
    let receipt = daemon
        .receive_notification(NotificationInput {
            app_name: "Shutdown fixture".into(),
            summary: "Keep pending attention".into(),
            actions: vec![NotificationAction::new("reply", "Reply")],
            resident: true,
            expire_timeout: canopa_notify::NotificationTimeout::Milliseconds(500),
            ..Default::default()
        })
        .await
        .unwrap();
    daemon
        .invoke_action(receipt.notification_id, "reply")
        .await
        .unwrap();
    let events = daemon.sender_events().await;
    let task = tokio::spawn({
        let daemon = daemon.clone();
        async move { daemon.shutdown().await }
    });
    tokio::time::timeout(Duration::from_secs(1), flushing.notified())
        .await
        .expect("shutdown did not drain the sender transport");
    assert!(presentation.0.load(Ordering::SeqCst));
    assert!(
        !task.is_finished(),
        "shutdown returned before the transport drained"
    );
    assert!(
        daemon
            .receive_notification(NotificationInput {
                app_name: "Too late".into(),
                summary: "Reject".into(),
                ..Default::default()
            })
            .await
            .is_err()
    );
    assert!(
        daemon
            .dismiss_notification(receipt.notification_id)
            .await
            .is_err()
    );
    assert!(daemon.set_do_not_disturb(true).await.is_err());
    released.add_permits(1);
    task.await.unwrap().unwrap();
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(
        daemon.sender_events().await,
        events,
        "a retired timer still emitted closure"
    );
    assert_eq!(
        daemon.snapshot().await.unwrap().active[0].status,
        NotificationStatus::Active
    );
}
