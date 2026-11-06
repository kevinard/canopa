use crate::support;

use canopa_notify::{Daemon, DaemonOptions, DeliveryOutcome, UserRule};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use support::notifications::notification;

struct Process(std::process::Child);

impl Drop for Process {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

fn assert_busy(error: anyhow::Error) {
    let database = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<sqlx::Error>())
        .and_then(sqlx::Error::as_database_error)
        .expect("expected a SQLite error, not an unrelated failure");
    assert_eq!(database.code().as_deref(), Some("5"), "{error:#}");
}

#[tokio::test]
async fn sqlite_busy_preserves_committed_state_and_effects_then_allows_retry() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("history.db");
    let daemon = Daemon::open(DaemonOptions::headless(path.clone()))
        .await
        .unwrap();
    let original = daemon
        .receive_notification(notification("Mail", "Keep me"))
        .await
        .unwrap();
    let before = daemon.snapshot().await.unwrap();
    let events = daemon.sender_events().await;
    let peer = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(&path))
        .await
        .unwrap();
    // A real competing SQLite writer; readers can still inspect committed state.
    sqlx::query("BEGIN IMMEDIATE").execute(&peer).await.unwrap();
    let mut replacement = notification("Mail", "Replacement");
    replacement.replaces_id = Some(original.notification_id);
    assert_busy(
        daemon
            .receive_notification(notification("Mail", "New"))
            .await
            .unwrap_err(),
    );
    assert_busy(
        daemon
            .receive_notification(replacement.clone())
            .await
            .unwrap_err(),
    );
    assert_busy(
        daemon
            .dismiss_notification(original.notification_id)
            .await
            .unwrap_err(),
    );
    assert_busy(daemon.set_do_not_disturb(true).await.unwrap_err());
    let rule = UserRule::for_app("Mail", DeliveryOutcome::Dropped);
    assert_busy(daemon.upsert_user_rule(rule.clone()).await.unwrap_err());
    assert_eq!(daemon.snapshot().await.unwrap(), before);
    assert_eq!(daemon.sender_events().await, events);
    assert!(!daemon.do_not_disturb().await.unwrap());
    assert!(daemon.user_rules().await.unwrap().is_empty());

    sqlx::query("ROLLBACK").execute(&peer).await.unwrap();
    peer.close().await;
    let retried = daemon.receive_notification(replacement).await.unwrap();
    assert_eq!(retried.notification_id, original.notification_id);
    assert_eq!(retried.delivery_outcome, DeliveryOutcome::Shown);
    daemon
        .dismiss_notification(original.notification_id)
        .await
        .unwrap();
    daemon.set_do_not_disturb(true).await.unwrap();
    daemon.upsert_user_rule(rule).await.unwrap();
    daemon.shutdown().await.unwrap();
    drop(daemon);
    let restarted = Daemon::open(DaemonOptions::headless(path)).await.unwrap();
    assert!(restarted.do_not_disturb().await.unwrap());
    assert_eq!(restarted.snapshot().await.unwrap().history.len(), 1);
    assert_eq!(
        restarted
            .receive_notification(notification("Mail", "Dropped after retry"))
            .await
            .unwrap()
            .delivery_outcome,
        DeliveryOutcome::Dropped
    );
}

#[tokio::test]
async fn killed_migration_rolls_back_and_restarts_with_history_and_policy_intact() {
    use std::time::{Duration, Instant};

    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let socket = temp.path().join("control.sock");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    let receipt = seed
        .receive_notification(notification("Mail", "Before migration"))
        .await
        .unwrap();
    seed.set_do_not_disturb(true).await.unwrap();
    let rule = UserRule::for_app("Mail", DeliveryOutcome::Silenced);
    seed.upsert_user_rule(rule.clone()).await.unwrap();
    drop(seed);
    let peer = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&database)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Delete),
        )
        .await
        .unwrap();
    // Reconstruct the released v1 schema. Hold a SHARED reader lock so the
    // child's real v2 DDL can run but its transaction cannot commit.
    sqlx::raw_sql(
        "DROP INDEX notification_threads_attention; DROP INDEX notification_threads_center;
        ALTER TABLE notification_threads DROP COLUMN requires_attention;
        ALTER TABLE notification_threads DROP COLUMN retained_bytes; PRAGMA user_version = 1;
        BEGIN; SELECT * FROM notification_threads;",
    )
    .execute(&peer)
    .await
    .unwrap();
    let start = || {
        Process(support::spawn_canopa_notify([
            "daemon",
            "--headless",
            "--database",
            support::path_arg(&database),
            "--socket",
            support::path_arg(&socket),
        ]))
    };
    let mut migrating = start();
    let journal = database.with_extension("db-journal");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            migrating.0.try_wait().unwrap().is_none(),
            "migration exited before interruption"
        );
        if std::fs::metadata(&journal).is_ok_and(|metadata| metadata.len() > 512) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "migration did not write its rollback journal"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    migrating.0.kill().unwrap();
    assert!(!migrating.0.wait().unwrap().success());
    sqlx::query("ROLLBACK").execute(&peer).await.unwrap();
    // A new read performs SQLite hot-journal recovery before any retry.
    assert_eq!(
        sqlx::query_scalar::<_, i64>("PRAGMA user_version")
            .fetch_one(&peer)
            .await
            .unwrap(),
        1
    );
    let columns: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_table_info('notification_threads')")
            .fetch_all(&peer)
            .await
            .unwrap();
    assert!(!columns.iter().any(|column| column == "requires_attention"));
    peer.close().await;

    // Exercise executable ownership and stale-socket recovery, then inspect
    // retained content through the public daemon boundary.
    let mut restarted = start();
    support::wait_for_status_with_child(&socket, &mut restarted.0);
    drop(restarted);
    let daemon = Daemon::open(DaemonOptions::headless(database))
        .await
        .unwrap();
    assert!(daemon.do_not_disturb().await.unwrap());
    assert_eq!(daemon.user_rules().await.unwrap(), vec![rule]);
    let snapshot = daemon.snapshot().await.unwrap();
    assert!(snapshot.active.is_empty());
    assert_eq!(snapshot.history.len(), 1);
    assert_eq!(snapshot.history[0].id, receipt.notification_id);
    assert_eq!(
        snapshot.history[0].accepted_notification.summary,
        "Before migration"
    );
    assert!(snapshot.history[0].requires_attention);
}
