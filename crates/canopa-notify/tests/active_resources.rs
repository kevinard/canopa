#[path = "support/daemon.rs"]
mod daemon_support;

use canopa_notify::{
    Daemon, DaemonOptions, DeliveryOutcome, NotificationAction, NotificationImageData,
    NotificationInput, NotificationLimitExceeded, NotificationStatus, NotificationTimeout,
    NotificationVisualReference, Urgency, UserRule,
};

fn text_notification(body_bytes: usize) -> NotificationInput {
    NotificationInput {
        body: "é".repeat(body_bytes / 2) + if body_bytes % 2 == 1 { "x" } else { "" },
        transient: true,
        expire_timeout: NotificationTimeout::Never,
        ..Default::default()
    }
}

#[tokio::test]
async fn live_text_budget_counts_accepted_representations_and_credits_only_committed_replacements()
{
    let daemon = daemon_support::open_headless_daemon().await;
    daemon.set_do_not_disturb(true).await.unwrap();
    let first = daemon
        .receive_notification(NotificationInput {
            transient: false,
            ..text_notification(65_536)
        })
        .await
        .unwrap();
    // Raw, plain and interpreted text each occupy 64 KiB: 85 receipts use 16,711,680 bytes.
    for _ in 1..85 {
        daemon
            .receive_notification(text_notification(65_536))
            .await
            .unwrap();
    }
    // The remaining 65,536 bytes: 3 * 21,845 body bytes + one summary byte.
    let tail = NotificationInput {
        transient: false,
        summary: "x".into(),
        ..text_notification(21_845)
    };
    let tail_receipt = daemon.receive_notification(tail.clone()).await.unwrap();
    let error = daemon
        .receive_notification(text_notification(1))
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "notification live text limit exceeded");
    assert!(error.is::<NotificationLimitExceeded>());
    assert!(daemon.sender_events().await.is_empty());

    assert_eq!(
        daemon
            .receive_notification(NotificationInput {
                replaces_id: Some(tail_receipt.notification_id),
                ..tail.clone()
            })
            .await
            .unwrap()
            .notification_id,
        tail_receipt.notification_id
    );
    let before = daemon.snapshot().await.unwrap();
    assert!(
        daemon
            .receive_notification(NotificationInput {
                replaces_id: Some(tail_receipt.notification_id),
                summary: "xx".into(),
                ..tail
            })
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
    assert_eq!(daemon.snapshot().await.unwrap(), before);

    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(first.notification_id),
            ..text_notification(65_535)
        })
        .await
        .unwrap();
    daemon
        .receive_notification(text_notification(1))
        .await
        .unwrap();
    assert!(
        daemon
            .receive_notification(text_notification(1))
            .await
            .is_err()
    );
    daemon
        .close_notification(tail_receipt.notification_id)
        .await
        .unwrap();
    // A closed target is new work, not replacement credit.
    assert!(
        daemon
            .receive_notification(NotificationInput {
                replaces_id: Some(first.notification_id),
                summary: "x".repeat(8192),
                ..text_notification(65_536)
            })
            .await
            .is_ok()
    );
    assert!(
        daemon
            .receive_notification(NotificationInput {
                replaces_id: Some(tail_receipt.notification_id),
                ..text_notification(65_536)
            })
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
}

fn notification(transient: bool) -> NotificationInput {
    NotificationInput {
        app_name: "Resources".into(),
        summary: "Live".into(),
        transient,
        expire_timeout: NotificationTimeout::Never,
        ..Default::default()
    }
}

#[tokio::test]
async fn live_text_budget_includes_identity_actions_and_visual_references() {
    let daemon = daemon_support::open_headless_daemon().await;
    daemon.set_do_not_disturb(true).await.unwrap();
    for _ in 0..85 {
        daemon
            .receive_notification(text_notification(65_536))
            .await
            .unwrap();
    }
    // 65,452 more logical bytes leave exactly 84 bytes below the 16 MiB ceiling.
    daemon
        .receive_notification(NotificationInput {
            summary: "x".into(),
            ..text_notification(21_817)
        })
        .await
        .unwrap();
    // 84 bytes: identities 11; summary 1; raw/plain/segmented body 37+4+10;
    // category 2; visuals 3+6; action key/label/icon 4+4+2.
    let rich = NotificationInput {
        app_name: "App".into(),
        desktop_entry: Some("org.test".into()),
        summary: "S".into(),
        body: "<b>T</b><img src=\"/x.png\" alt=\"ALT\"/>".into(),
        category: Some("im".into()),
        app_icon: NotificationVisualReference::icon_name("app"),
        image_reference: Some(NotificationVisualReference::LocalPath("/f.png".into())),
        actions: vec![
            NotificationAction::new("open", "Open")
                .with_icon(NotificationVisualReference::icon_name("go").unwrap()),
        ],
        expire_timeout: NotificationTimeout::Never,
        ..Default::default()
    };
    let receipt = daemon.receive_notification(rich.clone()).await.unwrap();
    assert!(
        daemon
            .receive_notification(text_notification(1))
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
    let before = daemon.snapshot().await.unwrap();
    assert!(
        daemon
            .receive_notification(NotificationInput {
                replaces_id: Some(receipt.notification_id),
                summary: "SS".into(),
                ..rich
            })
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
    assert_eq!(daemon.snapshot().await.unwrap(), before);
}

#[tokio::test]
async fn failed_replacements_preserve_text_capacity_and_recovered_history_is_not_live() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    daemon.set_do_not_disturb(true).await.unwrap();
    let first = daemon
        .receive_notification(NotificationInput {
            transient: false,
            ..text_notification(65_536)
        })
        .await
        .unwrap();
    for _ in 1..85 {
        daemon
            .receive_notification(text_notification(65_536))
            .await
            .unwrap();
    }
    daemon
        .receive_notification(NotificationInput {
            summary: "x".into(),
            ..text_notification(21_845)
        })
        .await
        .unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&database))
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_revision BEFORE UPDATE ON notification_threads BEGIN SELECT RAISE(ABORT, 'injected revision failure'); END")
        .execute(&pool).await.unwrap();
    let smaller = NotificationInput {
        replaces_id: Some(first.notification_id),
        ..text_notification(65_535)
    };
    let before = daemon.snapshot().await.unwrap();
    let error = daemon
        .receive_notification(smaller.clone())
        .await
        .unwrap_err();
    assert!(!error.is::<NotificationLimitExceeded>());
    assert_eq!(daemon.snapshot().await.unwrap(), before);
    assert!(
        daemon
            .receive_notification(text_notification(1))
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
    sqlx::query("DROP TRIGGER fail_revision")
        .execute(&pool)
        .await
        .unwrap();
    daemon.receive_notification(smaller).await.unwrap();
    sqlx::query("CREATE TRIGGER fail_revision BEFORE UPDATE ON notification_threads BEGIN SELECT RAISE(ABORT, 'injected revision failure'); END")
        .execute(&pool).await.unwrap();
    let before = daemon.snapshot().await.unwrap();
    let error = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(first.notification_id),
            ..text_notification(65_536)
        })
        .await
        .unwrap_err();
    assert!(!error.is::<NotificationLimitExceeded>());
    assert_eq!(daemon.snapshot().await.unwrap(), before);
    daemon
        .receive_notification(text_notification(1))
        .await
        .unwrap();
    assert!(
        daemon
            .receive_notification(text_notification(1))
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
    sqlx::query("DROP TRIGGER fail_revision")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    drop(daemon);

    let restarted = Daemon::open(DaemonOptions::headless(database))
        .await
        .unwrap();
    let snapshot = restarted.snapshot().await.unwrap();
    assert!(snapshot.active.is_empty());
    assert_eq!(snapshot.history[0].status, NotificationStatus::Stale);
    for _ in 0..85 {
        restarted
            .receive_notification(text_notification(65_536))
            .await
            .unwrap();
    }
    restarted
        .receive_notification(NotificationInput {
            summary: "x".into(),
            ..text_notification(21_845)
        })
        .await
        .unwrap();
    assert!(
        restarted
            .receive_notification(text_notification(1))
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
}

fn image_notification(bytes: usize) -> NotificationInput {
    NotificationInput {
        // Valid trailing/final-row padding must count, not just the four packed pixel bytes.
        image_data: Some(
            NotificationImageData::from_freedesktop(1, 1, 4, true, 8, 4, vec![0; bytes]).unwrap(),
        ),
        suppress_sound: true,
        ..notification(true)
    }
}

#[tokio::test]
async fn failed_writes_and_partial_clear_preserve_capacity_and_serialize_the_last_slot() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    daemon.set_do_not_disturb(true).await.unwrap();
    let blocked = daemon
        .receive_notification(NotificationInput {
            summary: "Fails".into(),
            ..notification(false)
        })
        .await
        .unwrap();
    daemon
        .receive_notification(notification(false))
        .await
        .unwrap();
    for _ in 2..1023 {
        daemon
            .receive_notification(notification(true))
            .await
            .unwrap();
    }
    // Fixture-only failure injection. All observations stay at the daemon seam.
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&database))
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_insert BEFORE INSERT ON notification_threads BEGIN SELECT RAISE(ABORT, 'injected receipt failure'); END")
        .execute(&pool).await.unwrap();
    let error = daemon
        .receive_notification(notification(false))
        .await
        .unwrap_err();
    assert!(!error.is::<NotificationLimitExceeded>());
    // Failed creation did not reserve the last slot.
    daemon
        .receive_notification(notification(true))
        .await
        .unwrap();
    assert!(
        daemon
            .receive_notification(notification(true))
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
    sqlx::query("CREATE TRIGGER fail_update BEFORE UPDATE ON notification_threads WHEN OLD.summary = 'Fails' BEGIN SELECT RAISE(ABORT, 'injected update failure'); END")
        .execute(&pool).await.unwrap();
    assert!(
        daemon
            .close_notification(blocked.notification_id)
            .await
            .is_err()
    );
    assert!(
        daemon
            .receive_notification(notification(true))
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
    assert!(daemon.clear_active_notifications().await.is_err());
    assert_eq!(daemon.snapshot().await.unwrap().active.len(), 1);
    // Only the committed retained dismissal freed capacity; the failed row and
    // transient threads remain live. Both contenders start before either finishes.
    let (left, right) = tokio::join!(
        daemon.receive_notification(notification(true)),
        daemon.receive_notification(notification(true)),
    );
    let results = [left, right];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert!(
        results
            .into_iter()
            .find_map(Result::err)
            .unwrap()
            .is::<NotificationLimitExceeded>()
    );
    sqlx::query("DROP TRIGGER fail_update")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TRIGGER fail_insert")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    daemon.clear_active_notifications().await.unwrap();
    for _ in 0..1024 {
        daemon
            .receive_notification(notification(true))
            .await
            .unwrap();
    }
    assert!(
        daemon
            .receive_notification(notification(true))
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
}

#[tokio::test]
async fn resident_actions_keep_capacity_until_dismissal_and_nonresident_actions_release_it() {
    let daemon = daemon_support::open_headless_daemon().await;
    daemon.set_do_not_disturb(true).await.unwrap();
    let action = NotificationInput {
        actions: vec![NotificationAction::new("open", "Open")],
        ..notification(true)
    };
    let resident = daemon
        .receive_notification(NotificationInput {
            resident: true,
            ..action.clone()
        })
        .await
        .unwrap();
    let nonresident = daemon.receive_notification(action).await.unwrap();
    for _ in 2..1024 {
        daemon
            .receive_notification(notification(true))
            .await
            .unwrap();
    }
    daemon
        .invoke_action(resident.notification_id, "open")
        .await
        .unwrap();
    assert!(
        daemon
            .receive_notification(notification(true))
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
    daemon
        .dismiss_notification(resident.notification_id)
        .await
        .unwrap();
    daemon
        .receive_notification(notification(true))
        .await
        .unwrap();
    assert!(
        daemon
            .receive_notification(notification(true))
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
    daemon
        .invoke_action(nonresident.notification_id, "open")
        .await
        .unwrap();
    daemon
        .receive_notification(notification(true))
        .await
        .unwrap();
    assert!(
        daemon
            .receive_notification(notification(true))
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
}

#[tokio::test]
async fn raw_image_budget_preserves_rejected_and_dropped_replacements_and_releases_silenced_images()
{
    let daemon = daemon_support::open_headless_daemon().await;
    let small = daemon
        .receive_notification(NotificationInput {
            transient: false,
            ..image_notification(4)
        })
        .await
        .unwrap();
    let mut ids = Vec::new();
    for bytes in [
        16 * 1024 * 1024,
        16 * 1024 * 1024,
        16 * 1024 * 1024,
        16 * 1024 * 1024 - 4,
    ] {
        ids.push(
            daemon
                .receive_notification(image_notification(bytes))
                .await
                .unwrap()
                .notification_id,
        );
    }
    let before = daemon.snapshot().await.unwrap();
    let error = daemon
        .receive_notification(image_notification(4))
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "notification live image limit exceeded");
    assert!(error.is::<NotificationLimitExceeded>());
    assert_eq!(daemon.snapshot().await.unwrap(), before);
    assert!(daemon.sender_events().await.is_empty());
    let banners = daemon.visible_banners().await;
    assert!(
        daemon
            .receive_notification(NotificationInput {
                replaces_id: Some(small.notification_id),
                ..image_notification(8)
            })
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
    assert_eq!(daemon.snapshot().await.unwrap(), before);
    assert_eq!(daemon.visible_banners().await, banners);
    // Replacing the same live image is permitted even when the budget is full.
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(ids[0]),
            ..image_notification(16 * 1024 * 1024)
        })
        .await
        .unwrap();
    daemon
        .upsert_user_rule(UserRule::for_app("Resources", DeliveryOutcome::Dropped))
        .await
        .unwrap();
    let dropped = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(ids[0]),
            ..image_notification(4)
        })
        .await
        .unwrap();
    assert_eq!(dropped.delivery_outcome, DeliveryOutcome::Dropped);
    daemon.remove_user_rule("Resources", None).await.unwrap();
    assert!(
        daemon
            .receive_notification(image_notification(4))
            .await
            .is_err()
    );

    daemon.set_do_not_disturb(true).await.unwrap();
    let silenced = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(ids[0]),
            ..image_notification(16 * 1024 * 1024)
        })
        .await
        .unwrap();
    assert_eq!(silenced.delivery_outcome, DeliveryOutcome::Silenced);
    // Silenced new work retains no raw image either, even when restoring the image budget.
    daemon
        .receive_notification(image_notification(16 * 1024 * 1024))
        .await
        .unwrap();
    daemon.set_do_not_disturb(false).await.unwrap();
    daemon
        .receive_notification(image_notification(16 * 1024 * 1024))
        .await
        .unwrap();
    assert!(
        daemon
            .receive_notification(image_notification(4))
            .await
            .is_err()
    );
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(ids[1]),
            ..notification(true)
        })
        .await
        .unwrap();
    daemon
        .receive_notification(image_notification(16 * 1024 * 1024))
        .await
        .unwrap();
    assert!(
        daemon
            .receive_notification(image_notification(4))
            .await
            .is_err()
    );
    daemon.expire_notification(ids[2]).await.unwrap();
    daemon
        .receive_notification(image_notification(16 * 1024 * 1024))
        .await
        .unwrap();
    assert!(
        daemon
            .receive_notification(image_notification(4))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn live_limit_covers_silenced_and_transient_notifications_without_evicting_existing_work() {
    let daemon = daemon_support::open_headless_daemon().await;
    daemon.set_do_not_disturb(true).await.unwrap();
    let retained = daemon
        .receive_notification(notification(false))
        .await
        .unwrap();
    let mut transient = retained.notification_id;
    for _ in 1..1024 {
        transient = daemon
            .receive_notification(notification(true))
            .await
            .unwrap()
            .notification_id;
    }
    let before = daemon.snapshot().await.unwrap();
    let error = daemon
        .receive_notification(NotificationInput {
            urgency: Urgency::Critical,
            ..notification(true)
        })
        .await
        .expect_err("accepted notification beyond the 1,024 live limit");
    assert!(error.downcast_ref::<NotificationLimitExceeded>().is_some());
    assert_eq!(error.to_string(), "notification live count limit exceeded");
    assert_eq!(daemon.snapshot().await.unwrap(), before);
    assert!(daemon.sender_events().await.is_empty());

    let revised = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(transient),
            summary: "Replacement still fits".into(),
            ..notification(true)
        })
        .await
        .unwrap();
    assert_eq!(revised.notification_id, transient);
    let dropped = daemon
        .receive_notification(NotificationInput::default())
        .await
        .unwrap();
    assert_eq!(dropped.delivery_outcome, DeliveryOutcome::Dropped);
    daemon
        .close_notification(retained.notification_id)
        .await
        .unwrap();
    daemon
        .receive_notification(notification(false))
        .await
        .unwrap();
    assert!(
        daemon
            .receive_notification(notification(true))
            .await
            .is_err()
    );
}
