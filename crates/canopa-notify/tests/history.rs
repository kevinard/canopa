#[path = "support/daemon.rs"]
mod daemon_support;
#[path = "support/notifications.rs"]
mod notifications;

use canopa_notify::{Daemon, DaemonOptions};
use notifications::notification;

#[tokio::test]
async fn inactive_history_is_pruned_by_count_while_active_threads_are_exempt() {
    let temp = tempfile::tempdir().unwrap();
    let options = DaemonOptions::headless(temp.path().join("history.db")).with_history_limit(2);
    let daemon = Daemon::open(options).await.unwrap();

    let mut ids = Vec::new();
    for index in 0..3 {
        ids.push(
            daemon
                .receive_notification(notification("App", format!("Active {index}")))
                .await
                .unwrap()
                .notification_id,
        );
    }

    assert_eq!(daemon.snapshot().await.unwrap().history.len(), 3);

    for id in ids {
        daemon.dismiss_notification(id).await.unwrap();
    }

    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.history.len(), 2);
    assert!(
        snapshot
            .history
            .iter()
            .all(|thread| thread.status.as_str() == "dismissed")
    );
}

#[tokio::test]
async fn inactive_history_is_pruned_by_count_at_daemon_startup() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()).with_history_limit(10))
        .await
        .unwrap();

    for index in 0..2 {
        let receipt = seed
            .receive_notification(notification("App", format!("Inactive {index}")))
            .await
            .unwrap();
        seed.dismiss_notification(receipt.notification_id)
            .await
            .unwrap();
    }
    assert_eq!(seed.snapshot().await.unwrap().history.len(), 2);
    drop(seed);

    let restarted = Daemon::open(DaemonOptions::headless(database).with_history_limit(1))
        .await
        .unwrap();

    let snapshot = restarted.snapshot().await.unwrap();
    assert_eq!(snapshot.history.len(), 1);
    assert!(snapshot.active.is_empty());
}

#[tokio::test]
async fn clear_active_and_delete_history_are_separate_operations() {
    let daemon = daemon_support::open_headless_daemon().await;

    daemon
        .receive_notification(notification("Mail", "Keep as history"))
        .await
        .unwrap();

    daemon.clear_active_notifications().await.unwrap();
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.active.len(), 0);
    assert_eq!(snapshot.history.len(), 1);

    daemon.delete_notification_history().await.unwrap();
    assert_eq!(daemon.snapshot().await.unwrap().history.len(), 0);
}
