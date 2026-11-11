//! Do Not Disturb timers, schedules and break-through, automatic suppression
//! and per-application preferences through the daemon and its control API.
use crate::notifications::notification;
use canopa_notify::{
    AppPermissionControl, AppPreferences, BreakThrough, ClockTime, ControlCommand, ControlReply,
    Daemon, DaemonOptions, DeliveryDecisionReason, DeliveryOutcome, DoNotDisturbPreferences,
    DoNotDisturbReason, DoNotDisturbRequest, DoNotDisturbSchedule, LockScreenVisibility,
    NotificationInput, NotifyControl, PreferenceSource, Suppression, SuppressionState, Urgency,
    Weekdays,
};
use std::time::Duration;

async fn daemon(temp: &tempfile::TempDir) -> Daemon {
    Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap()
}

fn categorized(app: &str, category: &str) -> NotificationInput {
    NotificationInput {
        category: Some(category.into()),
        ..notification(app, category)
    }
}

async fn outcome(daemon: &Daemon, input: NotificationInput) -> DeliveryOutcome {
    daemon
        .receive_notification(input)
        .await
        .unwrap()
        .delivery_outcome
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[tokio::test]
async fn timed_do_not_disturb_ends_by_itself_and_publishes_the_change() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = daemon(&temp).await;
    let control = NotifyControl::new(daemon.clone());
    let until = now_ms() + 400;
    let ControlReply::DoNotDisturbStatus(status) = control
        .execute(ControlCommand::RequestDoNotDisturb {
            request: DoNotDisturbRequest::Until { until },
        })
        .await
        .unwrap()
    else {
        panic!("unexpected reply")
    };
    assert_eq!(
        (status.active, status.reason, status.until),
        (true, Some(DoNotDisturbReason::Timer), Some(until))
    );
    let mut state = control.subscribe_notification_state();
    let summary = state.next_summary().await.unwrap().unwrap();
    assert_eq!(
        summary.do_not_disturb_reason,
        Some(DoNotDisturbReason::Timer)
    );
    assert_eq!(summary.do_not_disturb_until, Some(until));
    assert_eq!(
        outcome(&daemon, notification("Chat", "During")).await,
        DeliveryOutcome::Silenced
    );
    assert_eq!(
        outcome(
            &daemon,
            NotificationInput {
                urgency: Urgency::Critical,
                ..notification("Chat", "Alarm")
            }
        )
        .await,
        DeliveryOutcome::Shown
    );
    // The clock publishes the end without any command.
    let ended = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let summary = state.next_summary().await.unwrap().unwrap();
            if !summary.do_not_disturb {
                return summary;
            }
        }
    })
    .await
    .expect("timer end was not published");
    assert_eq!(ended.do_not_disturb_reason, None);
    assert_eq!(
        outcome(&daemon, notification("Chat", "After")).await,
        DeliveryOutcome::Shown
    );
    daemon.shutdown().await.unwrap();
}

#[tokio::test]
async fn timer_and_manual_state_survive_restart() {
    let temp = tempfile::tempdir().unwrap();
    let first = daemon(&temp).await;
    let status = first
        .request_do_not_disturb(DoNotDisturbRequest::For { minutes: 60 })
        .await
        .unwrap();
    first.shutdown().await.unwrap();
    drop(first);
    let second = daemon(&temp).await;
    assert_eq!(second.do_not_disturb_status().until, status.until);
    assert_eq!(
        second.do_not_disturb_status().reason,
        Some(DoNotDisturbReason::Timer)
    );
    second
        .request_do_not_disturb(DoNotDisturbRequest::On)
        .await
        .unwrap();
    second.shutdown().await.unwrap();
    drop(second);
    let third = daemon(&temp).await;
    let status = third.do_not_disturb_status();
    assert_eq!(
        (status.reason, status.until),
        (Some(DoNotDisturbReason::Manual), None)
    );
    assert!(third.change_do_not_disturb(None).await.is_ok_and(|on| !on));
    assert!(!third.do_not_disturb_status().active);
}

fn window_around_now(minutes_before: i64, minutes_after: i64) -> DoNotDisturbSchedule {
    use chrono::Timelike;
    let at = |offset: i64| {
        let time = chrono::Local::now() + chrono::Duration::minutes(offset);
        ClockTime::new(time.hour(), time.minute()).unwrap()
    };
    DoNotDisturbSchedule {
        start: at(-minutes_before),
        end: at(minutes_after),
        days: Weekdays::ALL,
    }
}

#[tokio::test]
async fn schedule_silences_and_off_skips_only_the_current_window() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = daemon(&temp).await;
    let control = NotifyControl::new(daemon.clone());
    let schedule = window_around_now(30, 30);
    control
        .execute(ControlCommand::SetDoNotDisturbPreferences {
            preferences: Some(DoNotDisturbPreferences {
                schedule: Some(schedule),
                ..Default::default()
            }),
        })
        .await
        .unwrap();
    let status = daemon.do_not_disturb_status();
    assert_eq!(status.reason, Some(DoNotDisturbReason::Schedule));
    assert_eq!(
        outcome(&daemon, notification("Chat", "Night")).await,
        DeliveryOutcome::Silenced
    );
    let log = daemon.delivery_decision_logs().await.pop().unwrap();
    assert_eq!(log.reason, DeliveryDecisionReason::DoNotDisturb);

    let off = daemon
        .request_do_not_disturb(DoNotDisturbRequest::Off)
        .await
        .unwrap();
    assert!(!off.active);
    assert_eq!(
        off.next_change, status.until,
        "skipped until the window ends"
    );
    assert_eq!(
        outcome(&daemon, notification("Chat", "Awake")).await,
        DeliveryOutcome::Shown
    );

    let ControlReply::NotificationPreferences(preferences) = control
        .execute(ControlCommand::NotificationPreferences)
        .await
        .unwrap()
    else {
        panic!("unexpected reply")
    };
    assert_eq!(preferences.do_not_disturb_source, PreferenceSource::User);
    assert_eq!(
        preferences.do_not_disturb_preferences.schedule,
        Some(schedule)
    );
    // Clearing returns to (absent) configuration.
    control
        .execute(ControlCommand::SetDoNotDisturbPreferences { preferences: None })
        .await
        .unwrap();
    assert!(!daemon.do_not_disturb_status().active);
}

#[tokio::test]
async fn configured_schedule_and_break_through_apply_and_ui_layer_wins() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("notify.kdl");
    let schedule = window_around_now(10, 10);
    std::fs::write(
        &config,
        format!(
            "do-not-disturb {{\n schedule from=\"{}\" to=\"{}\"\n break-through alarms=#true calls=#true\n}}\napp \"Clock\" lock-screen=\"content\"\n",
            schedule.start, schedule.end
        ),
    )
    .unwrap();
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db")).with_configuration_file(config),
    )
    .await
    .unwrap();
    assert_eq!(
        daemon.do_not_disturb_status().reason,
        Some(DoNotDisturbReason::Schedule)
    );
    assert_eq!(
        outcome(&daemon, categorized("Clock", "alarm.ringing")).await,
        DeliveryOutcome::Shown
    );
    assert_eq!(
        outcome(&daemon, categorized("Phone", "call.incoming")).await,
        DeliveryOutcome::Shown
    );
    assert_eq!(
        outcome(&daemon, categorized("Chat", "im.received")).await,
        DeliveryOutcome::Silenced
    );
    assert_eq!(
        outcome(&daemon, categorized("Phone", "call.missed")).await,
        DeliveryOutcome::Silenced
    );
    // A UI-managed copy without break-through replaces the configured copy.
    daemon
        .set_do_not_disturb_preferences(Some(DoNotDisturbPreferences {
            schedule: Some(schedule),
            break_through: BreakThrough {
                alarms: false,
                calls: true,
            },
            ..Default::default()
        }))
        .await
        .unwrap();
    assert_eq!(
        outcome(&daemon, categorized("Clock", "alarm.ringing")).await,
        DeliveryOutcome::Silenced
    );
    assert_eq!(
        outcome(&daemon, categorized("Phone", "call.incoming")).await,
        DeliveryOutcome::Shown
    );
    // An explicit drop rule still wins over break-through.
    daemon
        .upsert_user_rule(canopa_notify::UserRule::for_app(
            "Phone",
            DeliveryOutcome::Dropped,
        ))
        .await
        .unwrap();
    assert_eq!(
        outcome(&daemon, categorized("Phone", "call.incoming")).await,
        DeliveryOutcome::Dropped
    );
}

#[tokio::test]
async fn fullscreen_and_sharing_suppress_banners_except_critical() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = daemon(&temp).await;
    let control = NotifyControl::new(daemon.clone());
    let mut state = control.subscribe_notification_state();
    assert_eq!(
        state.next_summary().await.unwrap().unwrap().suppression,
        None
    );

    daemon.set_suppression(SuppressionState {
        fullscreen: true,
        screen_sharing: false,
    });
    assert_eq!(
        state.next_summary().await.unwrap().unwrap().suppression,
        Some(Suppression::Fullscreen)
    );
    assert_eq!(
        outcome(&daemon, notification("Chat", "Video")).await,
        DeliveryOutcome::Silenced
    );
    assert_eq!(
        daemon.delivery_decision_logs().await.pop().unwrap().reason,
        DeliveryDecisionReason::Fullscreen
    );
    let critical = NotificationInput {
        urgency: Urgency::Critical,
        ..notification("Battery", "Low")
    };
    assert_eq!(
        outcome(&daemon, critical.clone()).await,
        DeliveryOutcome::Shown
    );

    daemon.set_suppression(SuppressionState {
        fullscreen: true,
        screen_sharing: true,
    });
    assert_eq!(
        state.next_summary().await.unwrap().unwrap().suppression,
        Some(Suppression::ScreenSharing),
        "sharing takes precedence"
    );
    assert_eq!(
        outcome(&daemon, notification("Chat", "Secret")).await,
        DeliveryOutcome::Silenced
    );
    assert_eq!(
        daemon.delivery_decision_logs().await.pop().unwrap().reason,
        DeliveryDecisionReason::ScreenSharing
    );
    assert_eq!(outcome(&daemon, critical).await, DeliveryOutcome::Shown);

    // Preferences can turn each suppression off.
    daemon
        .set_do_not_disturb_preferences(Some(DoNotDisturbPreferences {
            suppress_fullscreen: false,
            suppress_screen_sharing: false,
            ..Default::default()
        }))
        .await
        .unwrap();
    assert_eq!(
        outcome(&daemon, notification("Chat", "Allowed")).await,
        DeliveryOutcome::Shown
    );
    daemon.set_suppression(SuppressionState::default());
    assert!(!daemon.do_not_disturb_status().active);
}

#[tokio::test]
async fn app_preferences_round_trip_through_the_control_api() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = daemon(&temp).await;
    let control = NotifyControl::new(daemon.clone());
    // Settings can configure an application before it has ever notified.
    let request = serde_json::json!({
        "command": "set-app-preferences",
        "app_identity": "org.gnome.Fractal",
        "delivery": "silence",
        "preferences": {"sound": false, "lock_screen": "content"}
    });
    let command: ControlCommand = serde_json::from_value(request).unwrap();
    assert_eq!(
        control.execute(command).await.unwrap(),
        ControlReply::CommandApplied
    );
    assert_eq!(
        outcome(
            &daemon,
            NotificationInput {
                desktop_entry: Some("org.gnome.Fractal".into()),
                ..notification("Fractal", "Hi")
            }
        )
        .await,
        DeliveryOutcome::Silenced
    );
    let ControlReply::NotificationPreferences(preferences) = control
        .execute(ControlCommand::NotificationPreferences)
        .await
        .unwrap()
    else {
        panic!("unexpected reply")
    };
    let entry = &preferences.apps[0];
    assert_eq!(entry.app_identity, "org.gnome.Fractal");
    assert_eq!(entry.delivery, Some(AppPermissionControl::Silence));
    assert!(!entry.sound);
    assert_eq!(entry.lock_screen, LockScreenVisibility::Content);
    assert_eq!(
        entry.user,
        AppPreferences {
            sound: Some(false),
            lock_screen: Some(LockScreenVisibility::Content)
        }
    );

    // Clearing every override removes the entry and the rule.
    control
        .execute(ControlCommand::SetAppPreferences {
            app_identity: "org.gnome.Fractal".into(),
            delivery: None,
            preferences: AppPreferences::default(),
        })
        .await
        .unwrap();
    let ControlReply::NotificationPreferences(preferences) = control
        .execute(ControlCommand::NotificationPreferences)
        .await
        .unwrap()
    else {
        panic!("unexpected reply")
    };
    assert!(preferences.apps.is_empty());
    assert!(
        control
            .execute(ControlCommand::SetAppPreferences {
                app_identity: " ".into(),
                delivery: None,
                preferences: AppPreferences::default(),
            })
            .await
            .is_err()
    );
}
