//! "Mark all read", "Clear all" with undo, and the lock-screen feed policy.
use crate::notifications::notification;
use canopa_notify::{
    AppPreferences, BreakThrough, ControlCommand, ControlReply, Daemon, DaemonOptions,
    DoNotDisturbPreferences, LockScreenVisibility, NotificationInput, NotificationTimeout,
    NotifyControl, Urgency,
};

async fn daemon(temp: &tempfile::TempDir) -> (Daemon, NotifyControl) {
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    (daemon.clone(), NotifyControl::new(daemon))
}

async fn center_titles(control: &NotifyControl) -> Vec<(String, bool)> {
    let ControlReply::NotificationCenterProjection(projection) = control
        .execute(ControlCommand::NotificationCenterProjection)
        .await
        .unwrap()
    else {
        panic!("unexpected reply")
    };
    let mut items: Vec<_> = projection
        .groups
        .into_iter()
        .flat_map(|group| group.items)
        .map(|item| (item.summary, item.requires_attention))
        .collect();
    items.sort();
    items
}

async fn unread(control: &NotifyControl) -> u64 {
    let mut state = control.subscribe_notification_state();
    state
        .next_summary()
        .await
        .unwrap()
        .unwrap()
        .requires_attention
}

fn live(app: &str, summary: &str) -> NotificationInput {
    NotificationInput {
        expire_timeout: NotificationTimeout::Never,
        ..notification(app, summary)
    }
}

#[tokio::test]
async fn mark_all_read_keeps_live_notifications_live() {
    let temp = tempfile::tempdir().unwrap();
    let (daemon, control) = daemon(&temp).await;
    let receipt = daemon
        .receive_notification(live("Chat", "Live"))
        .await
        .unwrap();
    daemon
        .receive_notification(live("Mail", "Other"))
        .await
        .unwrap();
    daemon
        .expire_notification(receipt.notification_id)
        .await
        .unwrap();
    assert_eq!(unread(&control).await, 2);
    assert_eq!(
        control.execute(ControlCommand::MarkAllRead).await.unwrap(),
        ControlReply::CommandApplied
    );
    assert_eq!(unread(&control).await, 0);
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        snapshot.active.len(),
        1,
        "the live notification is not dismissed"
    );
    assert!(daemon.sender_events().await.is_empty() || daemon
        .sender_events()
        .await
        .iter()
        .all(|event| !matches!(event, canopa_notify::SenderEvent::NotificationClosed { notification_id, .. } if *notification_id != receipt.notification_id)));
}

#[tokio::test]
async fn clear_all_hides_items_and_undo_restores_them_unread() {
    let temp = tempfile::tempdir().unwrap();
    let (daemon, control) = daemon(&temp).await;
    daemon
        .receive_notification(live("Chat", "Live"))
        .await
        .unwrap();
    let old = daemon
        .receive_notification(live("Mail", "Old"))
        .await
        .unwrap();
    daemon
        .dismiss_notification(old.notification_id)
        .await
        .unwrap();
    let unread_one = daemon
        .receive_notification(live("Mail", "Unread"))
        .await
        .unwrap();
    daemon
        .expire_notification(unread_one.notification_id)
        .await
        .unwrap();
    assert_eq!(center_titles(&control).await.len(), 3);

    let ControlReply::Cleared { undo } = control.execute(ControlCommand::ClearAll).await.unwrap()
    else {
        panic!("unexpected reply")
    };
    assert!(center_titles(&control).await.is_empty());
    assert_eq!(unread(&control).await, 0);
    assert!(
        daemon.snapshot().await.unwrap().active.is_empty(),
        "live items were dismissed"
    );
    assert_eq!(control.clear_undo(), Some(undo));

    // New items after a clear remain visible.
    daemon
        .receive_notification(live("Chat", "After"))
        .await
        .unwrap();
    assert_eq!(center_titles(&control).await, [("After".to_owned(), true)]);

    assert!(
        control
            .execute(ControlCommand::UndoClear { undo: undo + 1 })
            .await
            .is_err()
    );
    control
        .execute(ControlCommand::UndoClear { undo })
        .await
        .unwrap();
    let titles = center_titles(&control).await;
    assert_eq!(
        titles,
        [
            ("After".to_owned(), true),
            ("Live".to_owned(), true),
            ("Old".to_owned(), false),
            ("Unread".to_owned(), true),
        ]
    );
    assert_eq!(control.clear_undo(), None, "undo is single use");
    assert!(
        control
            .execute(ControlCommand::UndoClear { undo })
            .await
            .is_err()
    );

    // A cleared Center stays cleared across a restart.
    control.execute(ControlCommand::ClearAll).await.unwrap();
    daemon.shutdown().await.unwrap();
    drop((daemon, control));
    let (_daemon, control) = self::daemon(&temp).await;
    assert!(center_titles(&control).await.is_empty());
    assert_eq!(control.clear_undo(), None, "undo is runtime-only");
}

#[tokio::test]
async fn lock_screen_feed_shows_only_critical_items_without_content_by_default() {
    let temp = tempfile::tempdir().unwrap();
    let (daemon, _control) = daemon(&temp).await;
    daemon
        .receive_notification(live("Chat", "Normal secret"))
        .await
        .unwrap();
    daemon
        .receive_notification(NotificationInput {
            urgency: Urgency::Critical,
            ..live("Bank", "Code 123456")
        })
        .await
        .unwrap();
    let snapshot = daemon.lock_screen_snapshot().await.unwrap();
    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(snapshot.entries[0].line(), "Bank — New notification");
    assert!(snapshot.entries[0].critical);

    // An app rule can allow content, or hide the app entirely.
    daemon
        .set_app_preferences(
            "Bank",
            AppPreferences {
                sound: None,
                lock_screen: Some(LockScreenVisibility::Content),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        daemon.lock_screen_snapshot().await.unwrap().entries[0].line(),
        "Bank — Code 123456"
    );
    daemon
        .set_app_preferences(
            "Bank",
            AppPreferences {
                sound: None,
                lock_screen: Some(LockScreenVisibility::Hidden),
            },
        )
        .await
        .unwrap();
    assert!(
        daemon
            .lock_screen_snapshot()
            .await
            .unwrap()
            .entries
            .is_empty()
    );

    // Alarms appear only when they break through Do Not Disturb.
    daemon
        .receive_notification(NotificationInput {
            category: Some("alarm.ringing".into()),
            ..live("Clock", "Wake up")
        })
        .await
        .unwrap();
    assert!(
        daemon
            .lock_screen_snapshot()
            .await
            .unwrap()
            .entries
            .is_empty()
    );
    daemon
        .set_do_not_disturb_preferences(Some(DoNotDisturbPreferences {
            break_through: BreakThrough {
                alarms: true,
                calls: false,
            },
            ..Default::default()
        }))
        .await
        .unwrap();
    let snapshot = daemon.lock_screen_snapshot().await.unwrap();
    assert_eq!(snapshot.entries[0].line(), "Clock — New notification");
    assert!(!snapshot.entries[0].critical);
}

#[tokio::test]
async fn lock_screen_feed_is_bounded_and_streams_over_the_socket() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let temp = tempfile::tempdir().unwrap();
    let (daemon, _control) = daemon(&temp).await;
    for index in 0..6 {
        daemon
            .receive_notification(NotificationInput {
                urgency: Urgency::Critical,
                ..live("Pager", &format!("Alert {index}"))
            })
            .await
            .unwrap();
    }
    let snapshot = daemon.lock_screen_snapshot().await.unwrap();
    assert_eq!(
        snapshot.entries.len(),
        canopa_notify_client::MAX_LOCK_SCREEN_ENTRIES
    );
    assert_eq!(snapshot.more, 2);

    daemon.shutdown().await.unwrap();
    drop(daemon);

    // The daemon serves the same policy-filtered frames over its socket.
    let socket = temp.path().join("control.sock");
    struct Process(std::process::Child);
    impl Drop for Process {
        fn drop(&mut self) {
            self.0.kill().ok();
            self.0.wait().ok();
        }
    }
    let mut process = Process(crate::support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        socket.to_str().unwrap(),
        "--database",
        temp.path().join("history.db").to_str().unwrap(),
    ]));
    crate::support::wait_for_status_with_child(&socket, &mut process.0);
    let (mut subscription, first) = canopa_notify_client::LockScreenSubscription::connect(&socket)
        .await
        .unwrap();
    // Restart recovery makes earlier live items stale; unread critical items remain.
    assert_eq!(first.entries.len(), 4);
    assert_eq!(first.more, 2);
    assert!(
        first
            .entries
            .iter()
            .all(|entry| entry.text == "New notification")
    );
    let mut raw = tokio::net::UnixStream::connect(&socket).await.unwrap();
    raw.write_all(canopa_notify_client::LOCK_SCREEN_REQUEST.as_bytes())
        .await
        .unwrap();
    let mut buffer = vec![0; canopa_notify_client::MAX_LOCK_SCREEN_BYTES];
    let read = raw.read(&mut buffer).await.unwrap();
    let frame = std::str::from_utf8(&buffer[..read]).unwrap();
    assert!(frame.ends_with('\n') && !frame.contains("Alert"), "{frame}");
    drop(raw);
    drop(process);
    assert!(subscription.next().await.ok().flatten().is_none());
}

#[tokio::test]
async fn composing_holds_the_banner_and_sender_lock_hints_apply() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(
        DaemonOptions::graphical(
            temp.path().join("history.db"),
            canopa_notify::NoopBannerPresenter,
        )
        .with_notification_timeout_defaults(canopa_notify::NotificationTimeoutDefaults {
            low: Some(std::time::Duration::from_millis(80)),
            normal: Some(std::time::Duration::from_millis(80)),
            critical: None,
        }),
    )
    .await
    .unwrap();
    let receipt = daemon
        .receive_notification(NotificationInput {
            actions: vec![canopa_notify::NotificationAction::new("reply", "Reply")],
            presentation: canopa_notify::PresentationHints {
                keep_after_banner: true,
                reply: Some(canopa_notify::ReplyRequest {
                    action_key: "reply".into(),
                    placeholder: Some("Message Alice".into()),
                }),
                ..Default::default()
            },
            ..notification("Chat", "Alice")
        })
        .await
        .unwrap();
    let banner = daemon.visible_banners().await.remove(0);
    assert_eq!(
        banner.reply.as_ref().unwrap().placeholder.as_deref(),
        Some("Message Alice")
    );
    let target = banner.target();
    daemon
        .handle_banner_interaction(canopa_notify::BannerInteraction::Composing {
            target,
            composing: true,
        })
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    assert_eq!(
        daemon.visible_banners().await.len(),
        1,
        "held while composing"
    );
    daemon
        .handle_banner_interaction(canopa_notify::BannerInteraction::Composing {
            target,
            composing: false,
        })
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !daemon.visible_banners().await.is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("banner did not hide after composing ended");
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        snapshot.active[0].id, receipt.notification_id,
        "still live after hiding"
    );

    daemon
        .receive_notification(NotificationInput {
            urgency: Urgency::Critical,
            presentation: canopa_notify::PresentationHints {
                lock_screen: Some(LockScreenVisibility::Hidden),
                ..Default::default()
            },
            ..notification("Bank", "Secret")
        })
        .await
        .unwrap();
    assert!(
        daemon
            .lock_screen_snapshot()
            .await
            .unwrap()
            .entries
            .is_empty()
    );
    daemon.shutdown().await.unwrap();
}
