#[path = "support/daemon.rs"]
mod daemon_support;
#[path = "support/notifications.rs"]
mod notifications;

use canopa_notify::{Daemon, DaemonOptions};
use notifications::notification;

#[tokio::test]
async fn legacy_body_backfill_bounds_interpretation_without_rewriting_original_content() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    let receipt = seed
        .receive_notification(notification("Legacy", "Old body"))
        .await
        .unwrap();
    let raw_body = format!("{}éTAIL", "a".repeat(65_535));
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&database))
        .await
        .unwrap();
    // Model an old row awaiting interpreted-content backfill, not a new receipt.
    sqlx::query("UPDATE notification_threads SET raw_body = ?1, body = 'Legacy plain body', body_content_json = NULL WHERE id = ?2")
        .bind(&raw_body).bind(receipt.notification_id.get()).execute(&pool).await.unwrap();
    sqlx::query("PRAGMA user_version = 0")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    drop(seed);
    let restarted = Daemon::open(DaemonOptions::headless(database))
        .await
        .unwrap();
    let history = restarted.snapshot().await.unwrap().history;
    assert_eq!(history.len(), 1);
    let accepted = &history[0].accepted_notification;
    assert_eq!(accepted.body_content.plain_text().len(), 65_535);
    assert!(accepted.body_content.plain_text().ends_with('a'));
    assert_eq!(accepted.raw_body, raw_body);
    assert_eq!(accepted.body, "Legacy plain body");
}

#[tokio::test]
async fn legacy_zero_replacement_metadata_remains_readable_after_restart() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    let receipt = seed
        .receive_notification(notification("Legacy", "Keep this history"))
        .await
        .unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&database))
        .await
        .unwrap();
    // Previous library inputs could persist Some(0), meaning no live replacement.
    sqlx::query("UPDATE notification_threads SET replaces_id = 0 WHERE id = ?1")
        .bind(receipt.notification_id.get())
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    drop(seed);
    let restarted = Daemon::open(DaemonOptions::headless(database))
        .await
        .unwrap();
    let history = restarted.snapshot().await.unwrap().history;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, receipt.notification_id);
    assert_eq!(history[0].accepted_notification.replaces_id, None);
    assert_eq!(
        history[0].accepted_notification.summary,
        "Keep this history"
    );
    assert_eq!(history[0].status, canopa_notify::NotificationStatus::Stale);
}

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

#[tokio::test]
async fn partially_failed_clear_publishes_every_committed_closure() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    daemon
        .receive_notification(notification("App", "Fails"))
        .await
        .unwrap();
    let cleared = daemon
        .receive_notification(notification("App", "Clears first"))
        .await
        .unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&database))
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_one_clear BEFORE UPDATE ON notification_threads WHEN OLD.summary = 'Fails' BEGIN SELECT RAISE(ABORT, 'injected write failure'); END")
        .execute(&pool).await.unwrap();
    let result = daemon.clear_active_notifications().await;
    pool.close().await;
    assert!(result.is_err());
    let active = daemon.snapshot().await.unwrap().active;
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].accepted_notification.summary, "Fails");
    assert!(daemon.sender_events().await.iter().any(|event| matches!(event,
        canopa_notify::SenderEvent::NotificationClosed {notification_id, ..} if *notification_id == cleared.notification_id
    )), "committed closure lost its sender event");
    assert!(
        !daemon
            .visible_banners()
            .await
            .iter()
            .any(|banner| banner.notification_id == cleared.notification_id)
    );
}

#[tokio::test]
async fn future_database_schema_is_rejected_before_recovering_active_history() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    seed.receive_notification(notification("App", "Still active"))
        .await
        .unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&database))
        .await
        .unwrap();
    sqlx::query("PRAGMA user_version = 999")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    assert!(
        Daemon::open(DaemonOptions::headless(database))
            .await
            .is_err(),
        "future schema was accepted"
    );
    assert_eq!(seed.snapshot().await.unwrap().active.len(), 1);
}
