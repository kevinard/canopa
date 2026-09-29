#[path = "support/daemon.rs"]
mod daemon_support;
#[path = "support/notifications.rs"]
mod notifications;

use canopa_notify::{Daemon, DaemonOptions};
use notifications::notification;

#[tokio::test]
async fn retained_count_budget_preserves_pending_content_and_exempts_transient_notifications() {
    use canopa_notify::{
        ControlCommand, ControlReply, NotificationCenterAction, NotificationInput, NotifyControl,
    };
    let temp = tempfile::tempdir().unwrap();
    let daemon =
        Daemon::open(DaemonOptions::headless(temp.path().join("history.db")).with_history_limit(0))
            .await
            .unwrap();
    for index in 0..4096 {
        let receipt = daemon
            .receive_notification(notification("Mail", format!("Pending {index}")))
            .await
            .unwrap();
        daemon
            .expire_notification(receipt.notification_id)
            .await
            .unwrap();
    }
    let error = daemon
        .receive_notification(notification("Mail", "One too many"))
        .await
        .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<canopa_notify::NotificationLimitExceeded>()
            .unwrap()
            .to_string(),
        "notification retained count limit exceeded"
    );
    let transient = daemon
        .receive_notification(NotificationInput {
            transient: true,
            ..notification("Player", "Runtime only")
        })
        .await
        .unwrap();
    daemon
        .close_notification(transient.notification_id)
        .await
        .unwrap();
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.history.len(), 4096);
    assert!(
        snapshot
            .history
            .iter()
            .all(|thread| thread.requires_attention)
    );
    let control = NotifyControl::new(daemon.clone());
    let ControlReply::NotificationCenterBatch(batch) = control
        .execute(ControlCommand::NotificationCenterBatch {
            after: None,
            limit: 1,
        })
        .await
        .unwrap()
    else {
        panic!("center batch")
    };
    let target = batch.projection.groups[0].items[0].target.clone();
    control
        .execute(ControlCommand::InteractWithNotificationCenter {
            target,
            action: NotificationCenterAction::Acknowledge,
        })
        .await
        .unwrap();
    daemon
        .receive_notification(notification("Mail", "Capacity released"))
        .await
        .unwrap();
    assert_eq!(daemon.snapshot().await.unwrap().history.len(), 4096);
}

#[tokio::test]
async fn chronological_center_migration_preserves_history_and_sorts_by_date_before_id() {
    use canopa_notify::{ControlCommand, ControlReply, NotifyControl};
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    let newer = seed
        .receive_notification(notification("Mail", "Newer read message"))
        .await
        .unwrap();
    seed.dismiss_notification(newer.notification_id)
        .await
        .unwrap();
    let older = seed
        .receive_notification(notification("Mail", "Older unread message"))
        .await
        .unwrap();
    seed.expire_notification(older.notification_id)
        .await
        .unwrap();
    let before = seed.snapshot().await.unwrap();
    drop(seed);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&database))
        .await
        .unwrap();
    // Model a v2 database and a clock adjustment: the lower ID has the later date.
    sqlx::raw_sql("DROP INDEX notification_threads_center; CREATE INDEX notification_threads_center ON notification_threads(requires_attention DESC, created_at DESC, id DESC); PRAGMA user_version = 2;")
        .execute(&pool).await.unwrap();
    sqlx::query("UPDATE notification_threads SET created_at = (SELECT MAX(created_at) + 120 FROM notification_threads) WHERE id = ?1")
        .bind(newer.notification_id.get())
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let daemon = Daemon::open(DaemonOptions::headless(database))
        .await
        .unwrap();
    let after = daemon.snapshot().await.unwrap();
    assert_eq!(after.history.len(), before.history.len());
    for thread in &after.history {
        let original = before
            .history
            .iter()
            .find(|item| item.id == thread.id)
            .unwrap();
        assert_eq!(thread.accepted_notification, original.accepted_notification);
        assert_eq!(thread.requires_attention, original.requires_attention);
        assert_eq!(thread.status, original.status);
    }
    let control = NotifyControl::new(daemon);
    let mut cursor = None;
    for id in [newer.notification_id, older.notification_id] {
        let ControlReply::NotificationCenterBatch(batch) = control
            .execute(ControlCommand::NotificationCenterBatch {
                after: cursor,
                limit: 1,
            })
            .await
            .unwrap()
        else {
            panic!("Center batch")
        };
        assert_eq!(batch.projection.groups[0].items[0].notification_id, id);
        cursor = batch.last;
    }
}

#[tokio::test]
async fn migration_preserves_archive_and_recovers_only_old_active_attention() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    let archived = seed
        .receive_notification(notification("Mail", "Old expired archive"))
        .await
        .unwrap();
    seed.expire_notification(archived.notification_id)
        .await
        .unwrap();
    seed.receive_notification(notification("Mail", "Old active content"))
        .await
        .unwrap();
    drop(seed);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&database))
        .await
        .unwrap();
    sqlx::raw_sql("DROP INDEX notification_threads_attention; DROP INDEX notification_threads_center; ALTER TABLE notification_threads DROP COLUMN requires_attention; ALTER TABLE notification_threads DROP COLUMN retained_bytes; PRAGMA user_version = 1;")
        .execute(&pool).await.unwrap();
    pool.close().await;
    let daemon = Daemon::open(DaemonOptions::headless(database).with_history_limit(0))
        .await
        .unwrap();
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        snapshot.history.len(),
        1,
        "migration must keep the old archive meaning"
    );
    assert_eq!(
        snapshot.history[0].accepted_notification.summary,
        "Old active content"
    );
    assert!(snapshot.history[0].requires_attention);
    assert_eq!(
        snapshot.history[0].status,
        canopa_notify::NotificationStatus::Stale
    );
}

#[tokio::test]
async fn retained_byte_budget_rejects_atomically_and_acknowledgement_releases_capacity() {
    use canopa_notify::{
        ControlCommand, ControlReply, NotificationCenterAction, NotificationInput, NotifyControl,
    };
    let temp = tempfile::tempdir().unwrap();
    let daemon =
        Daemon::open(DaemonOptions::headless(temp.path().join("history.db")).with_history_limit(0))
            .await
            .unwrap();
    let keeper = daemon
        .receive_notification(notification("Mail", "Keeper"))
        .await
        .unwrap();
    let large = NotificationInput {
        body: "x".repeat(65_536),
        ..notification("Mail", "Large")
    };
    let mut rejected = false;
    let mut pending = 1;
    for _ in 0..400 {
        match daemon.receive_notification(large.clone()).await {
            Ok(receipt) => {
                daemon
                    .expire_notification(receipt.notification_id)
                    .await
                    .unwrap();
                pending += 1;
            }
            Err(error) => {
                assert!(
                    error
                        .downcast_ref::<canopa_notify::NotificationLimitExceeded>()
                        .is_some()
                );
                rejected = true;
                break;
            }
        }
    }
    assert!(
        rejected,
        "pending content must have a byte budget independent of live resources"
    );
    assert_eq!(daemon.snapshot().await.unwrap().history.len(), pending);
    let replacement = NotificationInput {
        replaces_id: Some(keeper.notification_id),
        ..large
    };
    assert!(
        daemon
            .receive_notification(replacement.clone())
            .await
            .is_err()
    );
    let current = daemon.snapshot().await.unwrap().active;
    assert_eq!(current[0].accepted_notification.summary, "Keeper");
    assert_eq!(current[0].revision, 1);
    let control = NotifyControl::new(daemon.clone());
    let ControlReply::NotificationCenterProjection(center) = control
        .execute(ControlCommand::NotificationCenterProjection)
        .await
        .unwrap()
    else {
        panic!("expected Center");
    };
    let expired = center
        .groups
        .into_iter()
        .flat_map(|group| group.items)
        .find(|item| item.notification_id != keeper.notification_id)
        .unwrap();
    control
        .execute(ControlCommand::InteractWithNotificationCenter {
            target: expired.target,
            action: NotificationCenterAction::Acknowledge,
        })
        .await
        .unwrap();
    assert_eq!(
        daemon
            .receive_notification(replacement)
            .await
            .unwrap()
            .notification_id,
        keeper.notification_id
    );
    assert_eq!(daemon.snapshot().await.unwrap().history.len(), pending - 1);
}

#[tokio::test]
async fn unacknowledged_expiry_and_restart_survive_zero_archive_retention() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let options = DaemonOptions::headless(database.clone()).with_history_limit(0);
    let daemon = Daemon::open(options).await.unwrap();
    let expired = daemon
        .receive_notification(notification("Mail", "Expired but unread"))
        .await
        .unwrap();
    daemon
        .expire_notification(expired.notification_id)
        .await
        .unwrap();
    assert_eq!(
        daemon.snapshot().await.unwrap().history.len(),
        1,
        "banner expiry must not permit archive pruning of pending attention"
    );
    daemon
        .receive_notification(notification("Mail", "Active at restart"))
        .await
        .unwrap();
    drop(daemon);
    let mut options = DaemonOptions::headless(database).with_history_limit(0);
    options.retention_days = 0;
    let restarted = Daemon::open(options).await.unwrap();
    let snapshot = restarted.snapshot().await.unwrap();
    assert!(snapshot.active.is_empty());
    assert_eq!(
        snapshot.history.len(),
        2,
        "restart must preserve retained attention"
    );
    let control = canopa_notify::NotifyControl::new(restarted.clone());
    let canopa_notify::ControlReply::NotificationCenterProjection(center) = control
        .execute(canopa_notify::ControlCommand::NotificationCenterProjection)
        .await
        .unwrap()
    else {
        panic!("expected Center");
    };
    assert_eq!(center.groups[0].items.len(), 2);
    assert!(
        center.groups[0]
            .items
            .iter()
            .all(|item| item.actions.is_empty() && !item.has_default_action)
    );
    restarted.delete_notification_history().await.unwrap();
    assert!(
        restarted.snapshot().await.unwrap().history.is_empty(),
        "deliberate bulk removal may remove pending inactive content"
    );
}

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
    sqlx::raw_sql("DROP INDEX notification_threads_attention; DROP INDEX notification_threads_center; ALTER TABLE notification_threads DROP COLUMN requires_attention; ALTER TABLE notification_threads DROP COLUMN retained_bytes;")
        .execute(&pool).await.unwrap();
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
