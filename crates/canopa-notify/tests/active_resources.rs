use crate::daemon_support;

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

/// A live set filled with critical notifications has nothing to retire, so the
/// count limit still refuses new work there; accounting tests rely on that.
fn critical(input: NotificationInput) -> NotificationInput {
    NotificationInput {
        urgency: Urgency::Critical,
        ..input
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
            ..critical(notification(false))
        })
        .await
        .unwrap();
    daemon
        .receive_notification(critical(notification(false)))
        .await
        .unwrap();
    for _ in 2..1023 {
        daemon
            .receive_notification(critical(notification(true)))
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
        .receive_notification(critical(notification(false)))
        .await
        .unwrap_err();
    assert!(!error.is::<NotificationLimitExceeded>());
    // Failed creation did not reserve the last slot.
    daemon
        .receive_notification(critical(notification(true)))
        .await
        .unwrap();
    assert!(
        daemon
            .receive_notification(critical(notification(true)))
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
            .receive_notification(critical(notification(true)))
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
    assert!(daemon.clear_active_notifications().await.is_err());
    assert_eq!(daemon.snapshot().await.unwrap().active.len(), 1);
    // Only the committed retained dismissal freed capacity; the failed row and
    // transient threads remain live. Both contenders start before either finishes.
    let (left, right) = tokio::join!(
        daemon.receive_notification(critical(notification(true))),
        daemon.receive_notification(critical(notification(true))),
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
            .receive_notification(critical(notification(true)))
            .await
            .unwrap();
    }
    assert!(
        daemon
            .receive_notification(critical(notification(true)))
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
        ..critical(notification(true))
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
            .receive_notification(critical(notification(true)))
            .await
            .unwrap();
    }
    daemon
        .invoke_action(resident.notification_id, "open")
        .await
        .unwrap();
    assert!(
        daemon
            .receive_notification(critical(notification(true)))
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
    daemon
        .dismiss_notification(resident.notification_id)
        .await
        .unwrap();
    daemon
        .receive_notification(critical(notification(true)))
        .await
        .unwrap();
    assert!(
        daemon
            .receive_notification(critical(notification(true)))
            .await
            .unwrap_err()
            .is::<NotificationLimitExceeded>()
    );
    daemon
        .invoke_action(nonresident.notification_id, "open")
        .await
        .unwrap();
    daemon
        .receive_notification(critical(notification(true)))
        .await
        .unwrap();
    assert!(
        daemon
            .receive_notification(critical(notification(true)))
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
async fn live_limit_retires_the_oldest_non_critical_notification_instead_of_refusing() {
    // A day of DND or "Save quietly" in Headless Mode: nothing expires by itself.
    let daemon = daemon_support::open_headless_daemon().await;
    daemon.set_do_not_disturb(true).await.unwrap();
    let mut events = daemon.subscribe_sender_events();
    let retained = daemon
        .receive_notification(notification(false))
        .await
        .unwrap();
    let mut transient = Vec::new();
    for _ in 1..1024 {
        transient.push(
            daemon
                .receive_notification(notification(true))
                .await
                .unwrap()
                .notification_id,
        );
    }
    assert_eq!(daemon.snapshot().await.unwrap().active.len(), 1);

    // The flood keeps going: the oldest non-critical thread is retired, here the
    // silenced retained one, and stays in the Center as unread history.
    let alarm = daemon
        .receive_notification(critical(notification(true)))
        .await
        .expect("a full live set must not refuse every application");
    let snapshot = daemon.snapshot().await.unwrap();
    assert!(snapshot.active.is_empty());
    let expired = snapshot
        .history
        .iter()
        .find(|thread| thread.id == retained.notification_id)
        .unwrap();
    assert_eq!(expired.status, NotificationStatus::Expired);
    assert!(
        expired.requires_attention,
        "retirement is not acknowledgement"
    );
    assert!(matches!(
        events.recv().await.unwrap(),
        canopa_notify::SenderEvent::NotificationClosed {
            notification_id,
            reason: canopa_notify::SenderCloseReason::Expired,
        } if notification_id == retained.notification_id
    ));

    // Next in line is the oldest transient thread, which leaves no history.
    let newest = daemon
        .receive_notification(notification(true))
        .await
        .unwrap();
    assert!(matches!(
        events.recv().await.unwrap(),
        canopa_notify::SenderEvent::NotificationClosed {
            notification_id,
            reason: canopa_notify::SenderCloseReason::Expired,
        } if notification_id == transient[0]
    ));
    assert!(daemon.close_notification(transient[0]).await.is_err());
    daemon
        .close_notification(alarm.notification_id)
        .await
        .unwrap();
    daemon
        .close_notification(newest.notification_id)
        .await
        .unwrap();

    // A live replacement and a policy drop need no slot and retire nothing.
    daemon
        .receive_notification(notification(true))
        .await
        .unwrap();
    daemon
        .receive_notification(notification(true))
        .await
        .unwrap();
    while let Ok(event) = events.try_recv() {
        drop(event);
    }
    let revised = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(transient[1022]),
            summary: "Replacement still fits".into(),
            ..notification(true)
        })
        .await
        .unwrap();
    assert_eq!(revised.notification_id, transient[1022]);
    assert!(
        events.try_recv().is_err(),
        "a live replacement retired another thread"
    );
    let dropped = daemon
        .receive_notification(NotificationInput::default())
        .await
        .unwrap();
    assert_eq!(dropped.delivery_outcome, DeliveryOutcome::Dropped);
    while let Ok(event) = events.try_recv() {
        assert!(
            matches!(
                event,
                canopa_notify::SenderEvent::NotificationClosed { notification_id, .. }
                    if notification_id == dropped.notification_id
            ),
            "a policy drop retired a live thread: {event:?}"
        );
    }
}

#[tokio::test]
async fn live_limit_refuses_only_when_every_live_notification_is_critical() {
    let daemon = daemon_support::open_headless_daemon().await;
    for _ in 0..1024 {
        daemon
            .receive_notification(critical(notification(true)))
            .await
            .unwrap();
    }
    let before = daemon.snapshot().await.unwrap();
    for input in [notification(true), critical(notification(false))] {
        let error = daemon
            .receive_notification(input)
            .await
            .expect_err("critical notifications are never retired to make room");
        assert_eq!(error.to_string(), "notification live count limit exceeded");
    }
    assert_eq!(daemon.snapshot().await.unwrap(), before);
    assert!(daemon.sender_events().await.is_empty());
}

#[tokio::test]
async fn silenced_flood_expires_like_banners_and_never_refuses_applications() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(
        DaemonOptions::graphical(
            temp.path().join("history.db"),
            canopa_notify::NoopBannerPresenter,
        )
        .with_notification_timeout_defaults(
            canopa_notify::NotificationTimeoutDefaults::new(
                Some(std::time::Duration::from_millis(20)),
                Some(std::time::Duration::from_millis(20)),
                None,
            ),
        ),
    )
    .await
    .unwrap();
    daemon.set_do_not_disturb(true).await.unwrap();
    let mut accepted = Vec::new();
    for index in 0..1100 {
        accepted.push(
            daemon
                .receive_notification(NotificationInput {
                    app_name: "Chat".into(),
                    summary: format!("Quiet {index}"),
                    ..Default::default()
                })
                .await
                .expect("silenced notifications must not exhaust the live limit")
                .notification_id,
        );
    }
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !daemon.snapshot().await.unwrap().active.is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("silenced notifications never expired");
    let history = daemon.snapshot().await.unwrap().history;
    assert_eq!(history.len(), accepted.len());
    assert!(history.iter().all(|thread| {
        thread.status == NotificationStatus::Expired
            && thread.delivery_outcome == DeliveryOutcome::Silenced
            && thread.requires_attention
    }));
}
