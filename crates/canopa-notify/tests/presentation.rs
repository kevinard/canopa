use crate::daemon_support;
use crate::notifications;

#[cfg(feature = "gui")]
use canopa_notify::IcedLayerShellBannerPresenter;
use canopa_notify::{
    BannerBodySegment, BannerInteraction, BannerKeyboardInteractivity, BannerLayer,
    BannerOutputSelection, BannerPresentation, BannerPresentationAcknowledgement, BannerPresenter,
    BannerSurfaceSpec, BannerView, BannerVisual, Daemon, DaemonOptions, DeliveryOutcome,
    NotificationAction, NotificationImageData, NotificationInput, NotificationThread,
    NotificationTimeout, NotificationTimeoutDefaults, NotificationVisualReference, SenderEvent,
    Urgency, UserRule,
};
#[cfg(feature = "gui")]
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
#[cfg(feature = "gui")]
use iced_layershell::settings::StartMode;
use notifications::{notification, notification_with_body};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Default)]
struct RecordingPresenter {
    updates: Arc<Mutex<Vec<BannerPresentation>>>,
}

impl RecordingPresenter {
    fn updates(&self) -> Vec<Vec<BannerView>> {
        self.updates
            .lock()
            .unwrap()
            .iter()
            .map(|update| update.banners.clone())
            .collect()
    }

    fn presentation_updates(&self) -> Vec<BannerPresentation> {
        self.updates.lock().unwrap().clone()
    }
}

impl BannerPresenter for RecordingPresenter {
    fn replace_visible_banners(&self, banners: Vec<BannerView>) -> anyhow::Result<()> {
        self.updates
            .lock()
            .unwrap()
            .push(BannerPresentation::visible_banners(banners));
        Ok(())
    }

    fn replace_banner_presentation(&self, presentation: BannerPresentation) -> anyhow::Result<()> {
        self.updates.lock().unwrap().push(presentation);
        Ok(())
    }
}

struct RejectSoundTheme;
impl canopa_notify::SoundPresenter for RejectSoundTheme {
    fn apply_sound_settings(
        &self,
        settings: canopa_notify::NotificationSoundSettings,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            settings.theme != "reject",
            "injected sound settings failure"
        );
        Ok(())
    }
    fn play_notification_sound(
        &self,
        _: canopa_notify::NotificationSoundPlayback,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn stop_all_sounds(&self) {}
    fn supports_sound(&self) -> bool {
        false
    }
}

#[tokio::test]
async fn appearance_reload_repaints_existing_banners_without_revising_notifications() {
    use canopa_theme::{Appearance, ColorScheme, Rgb};
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("canopa.kdl");
    std::fs::write(
        &config,
        "schema-version 1\nappearance { color-scheme \"light\"; }\n",
    )
    .unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(
        DaemonOptions::graphical(temp.path().join("history.db"), presenter.clone())
            .with_sound_presenter(RejectSoundTheme)
            .with_configuration_source(canopa_config::Request::common(&config)),
    )
    .await
    .unwrap();
    daemon
        .receive_notification(notification("Mail", "Still here"))
        .await
        .unwrap();
    let before = presenter.presentation_updates().last().unwrap().clone();
    assert_eq!(before.appearance.color_scheme, ColorScheme::Light);
    std::fs::write(
        &config,
        "schema-version 1\nappearance { accent \"#abcdef\"; }\n",
    )
    .unwrap();
    daemon.reload_configuration_source(None).await.unwrap();
    let after = presenter.presentation_updates().last().unwrap().clone();
    assert_eq!(before.banners, after.banners);
    assert_eq!(
        after.appearance,
        Appearance {
            accent: Some(Rgb(171, 205, 239)),
            ..Default::default()
        }
    );
    let committed = daemon.configuration_status();
    std::fs::write(
        &config,
        "schema-version 1\nappearance { color-scheme \"private-invalid\"; }\n",
    )
    .unwrap();
    assert!(daemon.reload_configuration_source(None).await.is_err());
    assert_eq!(presenter.presentation_updates().last().unwrap(), &after);
    assert_eq!(daemon.configuration_status().source, committed.source);
    assert_eq!(
        daemon.configuration_status().generation,
        committed.generation
    );
    assert!(
        !daemon
            .configuration_status()
            .error
            .unwrap()
            .contains("private-invalid")
    );
    std::fs::write(&config, "schema-version 1\nappearance { color-scheme \"light\"; }\nnotifications { sound theme=\"reject\"; }\n").unwrap();
    assert!(daemon.reload_configuration_source(None).await.is_err());
    assert_eq!(presenter.presentation_updates().last().unwrap(), &after);
    assert_eq!(
        daemon.configuration_status().generation,
        committed.generation
    );
}

#[tokio::test]
async fn icon_configuration_loads_and_reloads_without_reclassifying_notifications() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("notify.kdl");
    std::fs::write(&config, "icons theme=\"Birch\" scale=2\n").unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(
        DaemonOptions::graphical(temp.path().join("history.db"), presenter.clone())
            .with_sound_presenter(RejectSoundTheme)
            .with_configuration_file(config.clone()),
    )
    .await
    .expect("installed configuration must accept icon selection");
    daemon
        .receive_notification(notification("Mail", "First"))
        .await
        .unwrap();
    let first = presenter
        .presentation_updates()
        .last()
        .unwrap()
        .icon_settings
        .clone();
    assert_eq!(first.theme, "Birch");
    assert_eq!(first.scale, 2);
    assert_eq!(daemon.snapshot().await.unwrap().active.len(), 1);
    std::fs::write(&config, "icons theme=\"Other\" scale=1\n").unwrap();
    daemon.reload_configuration_file(&config).await.unwrap();
    assert_eq!(daemon.snapshot().await.unwrap().active.len(), 1);
    daemon
        .receive_notification(notification("Mail", "Second"))
        .await
        .unwrap();
    let second = presenter
        .presentation_updates()
        .last()
        .unwrap()
        .icon_settings
        .clone();
    assert_eq!(second.theme, "Other");
    assert_eq!(second.scale, 1);
    for invalid in [
        "icons scale=0",
        "icons scale=9",
        "icons scale=1.5",
        "icons theme=\"../other\"",
        "icons theme=\"\"",
        "icons surprise=1",
        "icons; icons",
        "icons { child; }",
    ] {
        let generation = daemon.configuration_status().generation;
        std::fs::write(&config, format!("{invalid}\n")).unwrap();
        assert!(
            daemon.reload_configuration_file(&config).await.is_err(),
            "{invalid}"
        );
        assert_eq!(daemon.configuration_status().generation, generation);
        daemon
            .receive_notification(notification("Mail", "Still configured"))
            .await
            .unwrap();
        assert_eq!(
            presenter
                .presentation_updates()
                .last()
                .unwrap()
                .icon_settings,
            second
        );
    }
    let generation = daemon.configuration_status().generation;
    std::fs::write(&config, "icons theme=\"Uncommitted\" scale=4\nsound theme=\"reject\"\nrule app=\"Mail\" outcome=\"drop\"\n").unwrap();
    assert!(daemon.reload_configuration_file(&config).await.is_err());
    assert_eq!(daemon.configuration_status().generation, generation);
    let receipt = daemon
        .receive_notification(notification(
            "Mail",
            "Still configured after install failure",
        ))
        .await
        .unwrap();
    assert_eq!(receipt.delivery_outcome, DeliveryOutcome::Shown);
    assert_eq!(
        presenter
            .presentation_updates()
            .last()
            .unwrap()
            .icon_settings,
        second
    );
}

#[tokio::test]
async fn queued_banner_action_cannot_invoke_the_same_key_on_replacement_content() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();
    let input = NotificationInput {
        actions: vec![NotificationAction::new("reply", "Reply to Alice")],
        expire_timeout: NotificationTimeout::Never,
        ..notification("Mail", "Alice")
    };
    let receipt = daemon.receive_notification(input.clone()).await.unwrap();
    let original = presenter.updates().last().unwrap()[0].clone();
    let interaction = BannerInteraction::InvokeAction {
        target: original.target(),
        action_key: "reply".into(),
        activation_token: Some("token-for-alice".into()),
    };
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(receipt.notification_id),
            summary: "Bob".into(),
            actions: vec![NotificationAction::new("reply", "Reply to Bob")],
            ..input
        })
        .await
        .unwrap();

    assert!(daemon.handle_banner_interaction(interaction).await.is_err());
    assert!(daemon.sender_events().await.is_empty());
    let active = daemon.snapshot().await.unwrap().active;
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].accepted_notification.summary, "Bob");

    let current = presenter.updates().last().unwrap()[0].clone();
    assert_ne!(original.target(), current.target());
    daemon
        .handle_banner_interaction(BannerInteraction::InvokeAction {
            target: current.target(),
            action_key: "reply".into(),
            activation_token: Some("token-for-bob".into()),
        })
        .await
        .unwrap();
    assert!(matches!(&daemon.sender_events().await[..], [
        SenderEvent::ActivationToken { activation_token, .. },
        SenderEvent::ActionInvoked { action_key, .. },
        SenderEvent::NotificationClosed { .. },
    ] if activation_token == "token-for-bob" && action_key == "reply"));
}

#[tokio::test]
async fn queued_banner_activation_and_dismissal_cannot_mutate_replaced_or_closed_content() {
    for transient in [false, true] {
        for has_default in [false, true] {
            for dismiss in [false, true] {
                let temp = tempfile::tempdir().unwrap();
                let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
                    .await
                    .unwrap();
                let input = NotificationInput {
                    transient,
                    expire_timeout: NotificationTimeout::Never,
                    actions: if has_default {
                        vec![NotificationAction::new("default", "Open")]
                    } else {
                        vec![]
                    },
                    ..notification("Mail", "Original")
                };
                let receipt = daemon.receive_notification(input.clone()).await.unwrap();
                let original = daemon.visible_banners().await[0].target();
                let interaction = |target| {
                    if dismiss {
                        BannerInteraction::Dismiss(target)
                    } else {
                        BannerInteraction::Activate {
                            target,
                            activation_token: Some("click-token".into()),
                        }
                    }
                };
                daemon
                    .receive_notification(NotificationInput {
                        replaces_id: Some(receipt.notification_id),
                        summary: "Replacement".into(),
                        ..input
                    })
                    .await
                    .unwrap();
                assert!(
                    daemon
                        .handle_banner_interaction(interaction(original))
                        .await
                        .is_err()
                );
                assert!(daemon.sender_events().await.is_empty());
                let current = daemon.visible_banners().await[0].target();
                daemon
                    .handle_banner_interaction(interaction(current))
                    .await
                    .unwrap();
                let events = daemon.sender_events().await;
                assert_eq!(events.len(), if has_default && !dismiss { 3 } else { 1 });
                assert!(matches!(
                    events.last(),
                    Some(SenderEvent::NotificationClosed {
                        reason: canopa_notify::SenderCloseReason::Dismissed,
                        ..
                    })
                ));
                assert!(
                    daemon
                        .handle_banner_interaction(interaction(current))
                        .await
                        .is_err()
                );
                assert_eq!(
                    daemon.sender_events().await,
                    events,
                    "duplicate interaction emitted effects"
                );
                assert!(daemon.visible_banners().await.is_empty());
                if transient {
                    assert!(daemon.snapshot().await.unwrap().history.is_empty());
                }
            }
        }
    }
}

#[tokio::test]
async fn resident_banner_actions_remain_repeatable_only_for_the_current_revision() {
    let daemon = daemon_support::open_headless_daemon().await;
    let input = NotificationInput {
        resident: true,
        transient: true,
        actions: vec![NotificationAction::new("next", "Next")],
        ..notification("Player", "First track")
    };
    let receipt = daemon.receive_notification(input.clone()).await.unwrap();
    let original = daemon.visible_banners().await[0].target();
    let interaction = BannerInteraction::InvokeAction {
        target: original,
        action_key: "next".into(),
        activation_token: None,
    };
    for _ in 0..2 {
        daemon
            .handle_banner_interaction(interaction.clone())
            .await
            .unwrap();
    }
    assert_eq!(daemon.sender_events().await.len(), 2);
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(receipt.notification_id),
            summary: "Second track".into(),
            ..input
        })
        .await
        .unwrap();
    assert!(daemon.handle_banner_interaction(interaction).await.is_err());
    assert_eq!(daemon.sender_events().await.len(), 2);
    assert_eq!(daemon.visible_banners().await[0].summary, "Second track");
}

#[tokio::test]
async fn banner_token_wait_cannot_commit_an_action_after_replacement() {
    struct WaitingToken(Arc<tokio::sync::Notify>);
    impl canopa_notify::ActivationTokenProvider for WaitingToken {
        fn available(&self) -> bool {
            true
        }
        fn activation_token(&self, _: String) -> canopa_notify::ActivationTokenFuture {
            let started = self.0.clone();
            Box::pin(async move {
                started.notify_one();
                std::future::pending().await
            })
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db"))
            .with_activation_token_provider(WaitingToken(started.clone())),
    )
    .await
    .unwrap();
    let input = NotificationInput {
        transient: true,
        actions: vec![NotificationAction::new("default", "Open")],
        ..notification("Mail", "Original")
    };
    let receipt = daemon.receive_notification(input.clone()).await.unwrap();
    let target = daemon.visible_banners().await[0].target();
    let action_daemon = daemon.clone();
    let action = tokio::spawn(async move {
        action_daemon
            .handle_banner_interaction(BannerInteraction::Activate {
                target,
                activation_token: None,
            })
            .await
    });
    tokio::time::timeout(Duration::from_secs(1), started.notified())
        .await
        .unwrap();
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(receipt.notification_id),
            summary: "Replacement".into(),
            ..input
        })
        .await
        .unwrap();
    assert!(action.await.unwrap().is_err());
    assert!(daemon.sender_events().await.is_empty());
    assert_eq!(daemon.visible_banners().await[0].summary, "Replacement");
}

#[tokio::test]
async fn renderer_failure_does_not_turn_a_committed_notification_into_a_failed_receipt() {
    struct OfflinePresenter;
    impl BannerPresenter for OfflinePresenter {
        fn replace_visible_banners(&self, _: Vec<BannerView>) -> anyhow::Result<()> {
            anyhow::bail!("renderer disconnected")
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        OfflinePresenter,
    ))
    .await
    .unwrap();
    let result = daemon
        .receive_notification(notification("Mail", "Retained despite renderer failure"))
        .await;
    assert!(result.is_ok(), "committed receipt failed: {result:?}");
    assert!(daemon.presentation_error().is_some());
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(snapshot.active.len(), 1);
    assert_eq!(snapshot.active[0].id, result.unwrap().notification_id);
    daemon
        .close_notification(snapshot.active[0].id)
        .await
        .unwrap();
    assert_eq!(daemon.sender_events().await.len(), 1);
}

#[derive(Clone, Default)]
struct DelayedAcknowledgementPresenter {
    updates: Arc<Mutex<Vec<BannerPresentation>>>,
    acknowledgements: Arc<Mutex<Vec<BannerPresentationAcknowledgement>>>,
}

impl DelayedAcknowledgementPresenter {
    fn acknowledge_latest(&self) {
        self.acknowledgements
            .lock()
            .unwrap()
            .pop()
            .unwrap()
            .acknowledge();
    }

    fn acknowledge_oldest(&self) {
        self.acknowledgements
            .lock()
            .unwrap()
            .remove(0)
            .acknowledge();
    }
}

impl BannerPresenter for DelayedAcknowledgementPresenter {
    fn replace_visible_banners(&self, banners: Vec<BannerView>) -> anyhow::Result<()> {
        self.updates
            .lock()
            .unwrap()
            .push(BannerPresentation::visible_banners(banners));
        Ok(())
    }

    fn replace_banner_presentation_with_acknowledgement(
        &self,
        presentation: BannerPresentation,
        acknowledgement: BannerPresentationAcknowledgement,
    ) -> anyhow::Result<()> {
        self.updates.lock().unwrap().push(presentation);
        self.acknowledgements.lock().unwrap().push(acknowledgement);
        Ok(())
    }
}

#[test]
fn notification_banner_surface_uses_focused_output_fallback_without_taking_focus() {
    let spec = BannerSurfaceSpec::notification_banner();

    assert_eq!(spec.output, BannerOutputSelection::FocusedWithFallback);
    assert_eq!(spec.layer, BannerLayer::Overlay);
    assert_eq!(
        spec.keyboard_interactivity,
        BannerKeyboardInteractivity::None
    );
    assert_eq!(spec.exclusive_zone, 0);
}

#[cfg(feature = "gui")]
#[test]
fn notification_banner_surface_maps_to_iced_layer_shell_settings() {
    let settings = IcedLayerShellBannerPresenter::layer_shell_settings(
        BannerSurfaceSpec::notification_banner(),
    );

    assert_eq!(
        settings.anchor,
        Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right
    );
    assert_eq!(
        settings.size,
        Some((0, 0)),
        "the compositor assigns both available dimensions"
    );
    assert_eq!(settings.margin, (12, 12, 12, 12));
    assert!(
        settings.events_transparent,
        "empty startup must not intercept desktop input"
    );
    assert_eq!(settings.layer, Layer::Overlay);
    assert_eq!(settings.keyboard_interactivity, KeyboardInteractivity::None);
    assert!(matches!(settings.start_mode, StartMode::Background));
    assert_eq!(settings.exclusive_zone, 0);
}

#[tokio::test]
async fn non_headless_daemon_presents_visible_banner_content() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    let receipt = daemon
        .receive_notification(notification_with_body(
            "Example Mail",
            "Inbox",
            "<b>One</b><br/>unread message",
        ))
        .await
        .unwrap();

    let updates = presenter.updates();
    let banners = updates.last().unwrap();
    assert_eq!(banners.len(), 1);
    assert_eq!(banners[0].notification_id, receipt.notification_id);
    assert_eq!(banners[0].summary, "Inbox");
    assert_eq!(banners[0].body, "One\nunread message");
    assert!(banners[0].body_content.text_runs()[0].style.bold);
}

#[tokio::test]
async fn transient_banner_overflow_keeps_every_center_action_without_starting_a_timeout() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        RecordingPresenter::default(),
    ))
    .await
    .unwrap();
    for summary in ["First critical", "Second critical", "Third critical"] {
        daemon
            .receive_notification(NotificationInput {
                urgency: Urgency::Critical,
                expire_timeout: NotificationTimeout::Never,
                ..notification("Critical", summary)
            })
            .await
            .unwrap();
    }
    let receipt = daemon
        .receive_notification(NotificationInput {
            transient: true,
            expire_timeout: NotificationTimeout::Milliseconds(20),
            actions: ["one", "two", "three", "four"]
                .into_iter()
                .map(|key| NotificationAction::new(key, format!("Long action label: {key}")))
                .collect(),
            ..notification_with_body(
                "Preview",
                "Overflow",
                "Before <img src=\"/tmp/preview.png\" alt=\"preview\"/> after",
            )
        })
        .await
        .unwrap();
    assert_eq!(daemon.visible_banners().await.len(), 3);
    assert!(!is_visible(
        &daemon.visible_banners().await,
        receipt.notification_id
    ));
    let control = canopa_notify::NotifyControl::new(daemon.clone());
    let mut events = daemon.subscribe_sender_events();
    let canopa_notify::ControlReply::NotificationCenterProjection(projection) = control
        .execute(canopa_notify::ControlCommand::NotificationCenterProjection)
        .await
        .unwrap()
    else {
        panic!("expected center projection")
    };
    let item = &projection
        .groups
        .iter()
        .find(|group| group.app_identity == "Preview")
        .unwrap()
        .items[0];
    assert!(item.transient);
    assert_eq!(item.body, "Before preview after");
    assert_eq!(
        item.actions
            .iter()
            .map(|action| action.key.as_str())
            .collect::<Vec<_>>(),
        ["one", "two", "three", "four"]
    );
    // Negative timing assertion: opening the center must not acknowledge an undisplayed banner.
    assert!(
        tokio::time::timeout(Duration::from_millis(60), events.recv())
            .await
            .is_err()
    );
    control
        .execute(canopa_notify::ControlCommand::InvokeNotificationAction {
            notification_id: item.notification_id,
            action_key: item.actions[3].key.clone(),
        })
        .await
        .unwrap();
    assert!(
        matches!(tokio::time::timeout(Duration::from_secs(2), events.recv()).await.unwrap().unwrap(), SenderEvent::ActionInvoked {
        notification_id, action_key,
    } if notification_id == receipt.notification_id && action_key == "four")
    );
    assert!(
        matches!(tokio::time::timeout(Duration::from_secs(2), events.recv()).await.unwrap().unwrap(), SenderEvent::NotificationClosed {
        notification_id, reason: canopa_notify::SenderCloseReason::Dismissed,
    } if notification_id == receipt.notification_id)
    );
    assert_eq!(daemon.snapshot().await.unwrap().history.len(), 3);
}

#[tokio::test]
async fn transient_center_item_disappears_after_display_acknowledged_expiry() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = DelayedAcknowledgementPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();
    let receipt = daemon
        .receive_notification(NotificationInput {
            transient: true,
            expire_timeout: NotificationTimeout::Milliseconds(20),
            ..notification("Preview", "Waiting for actual display")
        })
        .await
        .unwrap();
    let control = canopa_notify::NotifyControl::new(daemon.clone());
    let mut subscription = control.subscribe_notification_state();
    let projection = subscription.next_projection().await.unwrap().unwrap();
    assert_eq!(
        projection.groups[0].items[0].notification_id,
        receipt.notification_id
    );
    assert!(projection.groups[0].items[0].transient);
    let mut events = daemon.subscribe_sender_events();
    assert!(
        tokio::time::timeout(Duration::from_millis(60), events.recv())
            .await
            .is_err()
    );
    presenter.acknowledge_latest();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), events.recv()).await.unwrap().unwrap(),
        SenderEvent::NotificationClosed { notification_id, reason: canopa_notify::SenderCloseReason::Expired }
            if notification_id == receipt.notification_id
    ));
    let projection = tokio::time::timeout(Duration::from_secs(2), subscription.next_projection())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(projection.groups.is_empty());
    assert!(daemon.snapshot().await.unwrap().history.is_empty());
    assert!(daemon.visible_banners().await.is_empty());
}

#[tokio::test]
async fn transient_notification_uses_banner_runtime_without_history() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter,
    ))
    .await
    .unwrap();

    let receipt = daemon
        .receive_notification(NotificationInput {
            transient: true,
            expire_timeout: NotificationTimeout::Never,
            image_data: Some(test_image_data()),
            ..notification_with_body("Screenshot", "Captured", "Runtime preview")
        })
        .await
        .unwrap();

    assert!(is_visible(
        &daemon.visible_banners().await,
        receipt.notification_id
    ));
    assert!(daemon.visible_banners().await[0].image.is_some());
    assert!(daemon.snapshot().await.unwrap().history.is_empty());

    daemon
        .dismiss_notification(receipt.notification_id)
        .await
        .unwrap();
    assert!(daemon.visible_banners().await.is_empty());
    assert!(daemon.snapshot().await.unwrap().history.is_empty());
}

#[tokio::test]
async fn non_headless_daemon_presents_ordered_body_image_segments() {
    let temp = tempfile::tempdir().unwrap();
    let body_image_path = temp.path().join("chart.png");
    std::fs::write(&body_image_path, test_png_bytes()).unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    daemon
        .receive_notification(notification_with_body(
            "Reporter",
            "Report",
            format!(
                r#"Before <img src="{}" alt="chart"/> after"#,
                path_string(&body_image_path)
            ),
        ))
        .await
        .unwrap();

    let updates = presenter.updates();
    let banner = &updates.last().unwrap()[0];

    assert!(banner.image.is_none());
    assert_eq!(banner.body, "Before chart after");
    assert_eq!(
        banner.body_segments,
        vec![
            BannerBodySegment::plain_text("Before "),
            BannerBodySegment::image(BannerVisual::LocalPath {
                path: path_string(&body_image_path),
            }),
            BannerBodySegment::plain_text(" after"),
        ]
    );
}

#[tokio::test]
async fn non_headless_daemon_preserves_missing_body_image_reference_and_plain_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let missing_body_image_path = temp.path().join("missing.png");
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    daemon
        .receive_notification(notification_with_body(
            "Reporter",
            "Report",
            format!(
                r#"Before <img src="{}" alt="missing chart"/> after"#,
                path_string(&missing_body_image_path)
            ),
        ))
        .await
        .unwrap();

    let updates = presenter.updates();
    let banner = &updates.last().unwrap()[0];

    assert_eq!(banner.body, "Before missing chart after");
    assert_eq!(
        banner.body_segments,
        vec![
            BannerBodySegment::plain_text("Before "),
            BannerBodySegment::image(BannerVisual::LocalPath {
                path: path_string(&missing_body_image_path)
            }),
            BannerBodySegment::plain_text(" after")
        ]
    );
}

#[tokio::test]
async fn non_headless_daemon_leaves_file_size_validation_to_asset_preparation() {
    let temp = tempfile::tempdir().unwrap();
    let oversized_body_image_path = temp.path().join("oversized.png");
    std::fs::File::create(&oversized_body_image_path)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    daemon
        .receive_notification(notification_with_body(
            "Reporter",
            "Report",
            format!(
                r#"<img src="{}" alt="oversized chart"/>"#,
                path_string(&oversized_body_image_path)
            ),
        ))
        .await
        .unwrap();

    let updates = presenter.updates();
    let banner = &updates.last().unwrap()[0];

    assert_eq!(banner.body, "oversized chart");
    assert_eq!(
        banner.body_segments,
        vec![BannerBodySegment::image(BannerVisual::LocalPath {
            path: path_string(&oversized_body_image_path)
        })]
    );
}

#[tokio::test]
async fn non_headless_daemon_presents_body_image_only_banner_without_alt() {
    let temp = tempfile::tempdir().unwrap();
    let body_image_path = temp.path().join("chart.jpg");
    std::fs::write(&body_image_path, test_png_bytes()).unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    daemon
        .receive_notification(notification_with_body(
            "Reporter",
            "   ",
            format!(r#"<img src="file://{}"/>"#, path_string(&body_image_path)),
        ))
        .await
        .unwrap();

    let updates = presenter.updates();
    let banner = &updates.last().unwrap()[0];

    assert_eq!(banner.body, "");
    assert_eq!(
        banner.body_segments,
        vec![BannerBodySegment::image(BannerVisual::LocalPath {
            path: path_string(&body_image_path),
        })]
    );
}

#[tokio::test]
async fn body_embedded_images_do_not_replace_notification_visual_slots() {
    let temp = tempfile::tempdir().unwrap();
    let app_icon_path = temp.path().join("app.png");
    let notification_image_path = temp.path().join("photo.jpg");
    let body_image_path = temp.path().join("chart.png");
    std::fs::write(&app_icon_path, test_png_bytes()).unwrap();
    std::fs::write(&notification_image_path, test_png_bytes()).unwrap();
    std::fs::write(&body_image_path, test_png_bytes()).unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    daemon
        .receive_notification(NotificationInput {
            app_icon: NotificationVisualReference::local_path(path_string(&app_icon_path)),
            image_reference: NotificationVisualReference::local_path(path_string(
                &notification_image_path,
            )),
            ..notification_with_body(
                "Reporter",
                "Report",
                format!(
                    r#"<img src="{}" alt="chart"/>"#,
                    path_string(&body_image_path)
                ),
            )
        })
        .await
        .unwrap();

    let updates = presenter.updates();
    let banner = &updates.last().unwrap()[0];

    assert!(matches!(
        banner.app_icon.as_ref(),
        Some(BannerVisual::LocalPath { path }) if path == &path_string(&app_icon_path)
    ));
    assert!(matches!(
        banner.image.as_ref(),
        Some(BannerVisual::LocalPath { path }) if path == &path_string(&notification_image_path)
    ));
    assert_eq!(
        banner.body_segments,
        vec![BannerBodySegment::image(BannerVisual::LocalPath {
            path: path_string(&body_image_path),
        })]
    );
}

#[tokio::test]
async fn replacement_without_body_images_clears_previous_body_image_segments() {
    let temp = tempfile::tempdir().unwrap();
    let body_image_path = temp.path().join("chart.png");
    std::fs::write(&body_image_path, test_png_bytes()).unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    let first = daemon
        .receive_notification(notification_with_body(
            "Reporter",
            "Report",
            format!(
                r#"<img src="{}" alt="chart"/>"#,
                path_string(&body_image_path)
            ),
        ))
        .await
        .unwrap();

    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(first.notification_id),
            ..notification_with_body("Reporter", "Report", "Text only")
        })
        .await
        .unwrap();

    let updates = presenter.updates();
    let banner = &updates.last().unwrap()[0];

    assert_eq!(banner.notification_id, first.notification_id);
    assert_eq!(
        banner.body_segments,
        vec![BannerBodySegment::plain_text("Text only")]
    );
}

#[tokio::test]
async fn non_headless_daemon_presents_runtime_notification_image_data_without_persisting_payload() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    daemon
        .receive_notification(NotificationInput {
            image_data: Some(test_image_data()),
            ..notification("Photos", "Screenshot")
        })
        .await
        .unwrap();

    let updates = presenter.updates();
    let banner = &updates.last().unwrap()[0];
    let canopa_notify::BannerVisual::Rgba {
        width,
        height,
        pixels,
    } = banner.image.as_ref().unwrap()
    else {
        panic!("expected raw image visual");
    };
    assert_eq!((*width, *height), (1, 1));
    assert_eq!(pixels, &[255, 0, 0, 255]);

    let snapshot = daemon.snapshot().await.unwrap();
    assert!(!snapshot.active[0].accepted_notification.has_image_payload);
}

#[tokio::test]
async fn replacement_without_visuals_clears_previous_runtime_image() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    let first = daemon
        .receive_notification(NotificationInput {
            image_data: Some(test_image_data()),
            ..notification("Photos", "Screenshot")
        })
        .await
        .unwrap();

    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(first.notification_id),
            ..notification("Photos", "Screenshot updated")
        })
        .await
        .unwrap();

    let updates = presenter.updates();
    let banner = &updates.last().unwrap()[0];
    assert_eq!(banner.notification_id, first.notification_id);
    assert!(banner.image.is_none());
}

#[tokio::test]
async fn non_headless_daemon_presents_local_visual_references() {
    let temp = tempfile::tempdir().unwrap();
    let app_icon_path = temp.path().join("app.png");
    let image_path = temp.path().join("photo.jpg");
    std::fs::write(&app_icon_path, [0_u8]).unwrap();
    std::fs::write(&image_path, [0_u8]).unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    daemon
        .receive_notification(NotificationInput {
            app_icon: NotificationVisualReference::local_path(path_string(&app_icon_path)),
            image_reference: NotificationVisualReference::local_path(path_string(&image_path)),
            ..notification("Photos", "Local image")
        })
        .await
        .unwrap();

    let updates = presenter.updates();
    let banner = &updates.last().unwrap()[0];
    assert!(matches!(
        banner.app_icon.as_ref(),
        Some(canopa_notify::BannerVisual::LocalPath { path }) if path == &path_string(&app_icon_path)
    ));
    assert!(matches!(
        banner.image.as_ref(),
        Some(canopa_notify::BannerVisual::LocalPath { path }) if path == &path_string(&image_path)
    ));
}

#[tokio::test]
async fn non_headless_daemon_replaces_visible_banner_content_in_place() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    let first = daemon
        .receive_notification(notification_with_body("Downloader", "Download", "Starting"))
        .await
        .unwrap();

    let replacement = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(first.notification_id),
            progress: Some(100),
            ..notification_with_body("Downloader", "Download", "Complete")
        })
        .await
        .unwrap();

    assert_eq!(replacement.notification_id, first.notification_id);

    let updates = presenter.updates();
    let banners = updates.last().unwrap();
    assert_eq!(banners.len(), 1);
    assert_eq!(banners[0].notification_id, first.notification_id);
    assert_eq!(banners[0].summary, "Download");
    assert_eq!(banners[0].body, "Complete");
    assert_eq!(banners[0].progress, Some(100));
}

#[tokio::test]
async fn replacement_updates_existing_visible_banner_without_reordering_stack() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    let first = daemon
        .receive_notification(notification("Chat", "Message 1"))
        .await
        .unwrap();
    daemon
        .receive_notification(notification("Chat", "Message 2"))
        .await
        .unwrap();
    daemon
        .receive_notification(notification("Chat", "Message 3"))
        .await
        .unwrap();

    let updates = presenter.updates();
    let before_ids = visible_ids(updates.last().unwrap());

    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(first.notification_id),
            ..notification("Chat", "Message 1 edited")
        })
        .await
        .unwrap();

    let updates = presenter.updates();
    let banners = updates.last().unwrap();
    assert_eq!(visible_ids(banners), before_ids);
    assert_eq!(
        banners
            .iter()
            .find(|banner| banner.notification_id == first.notification_id)
            .unwrap()
            .summary,
        "Message 1 edited"
    );
}

#[tokio::test]
async fn headless_daemon_keeps_banner_state_without_presenting_surfaces() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db"))
            .with_banner_presenter(presenter.clone()),
    )
    .await
    .unwrap();

    daemon
        .receive_notification(notification("Chat", "Stored but not rendered"))
        .await
        .unwrap();

    assert_eq!(daemon.visible_banners().await.len(), 1);
    assert!(presenter.updates().is_empty());
}

#[tokio::test]
async fn headless_notification_with_finite_timeout_remains_active_without_sender_close() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();

    let receipt = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(10),
            ..notification("Calendar", "Not displayed")
        })
        .await
        .unwrap();

    assert_active_without_sender_close_after(
        &daemon,
        receipt.notification_id,
        Duration::from_millis(30),
    )
    .await;
}

#[tokio::test]
async fn visible_normal_banners_expire_after_configured_timeout() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(
        DaemonOptions::graphical(
            temp.path().join("history.db"),
            RecordingPresenter::default(),
        )
        .with_notification_timeout_defaults(NotificationTimeoutDefaults::new(
            Some(Duration::from_millis(10)),
            Some(Duration::from_millis(10)),
            None,
        )),
    )
    .await
    .unwrap();

    let receipt = daemon
        .receive_notification(notification("Calendar", "Soon"))
        .await
        .unwrap();

    assert!(is_visible(
        &daemon.visible_banners().await,
        receipt.notification_id
    ));

    tokio::time::sleep(Duration::from_millis(30)).await;

    assert!(!is_visible(
        &daemon.visible_banners().await,
        receipt.notification_id
    ));
    let snapshot = daemon.snapshot().await.unwrap();
    let thread = snapshot
        .history
        .iter()
        .find(|thread| thread.id == receipt.notification_id)
        .unwrap();
    assert_eq!(thread.status.as_str(), "expired");
}

#[tokio::test]
async fn delayed_display_acknowledgement_cannot_start_a_cancelled_revisions_timeout() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = DelayedAcknowledgementPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();
    let input = NotificationInput {
        expire_timeout: NotificationTimeout::Milliseconds(20),
        ..notification("Mail", "Original")
    };
    let receipt = daemon.receive_notification(input.clone()).await.unwrap();
    let original_target = presenter.updates.lock().unwrap().last().unwrap().banners[0].target();
    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(receipt.notification_id),
            summary: "Replacement".into(),
            ..input
        })
        .await
        .unwrap();
    presenter.acknowledgements.lock().unwrap()[0].acknowledge_banner(original_target);
    presenter.acknowledge_oldest();
    assert_active_without_sender_close_after(
        &daemon,
        receipt.notification_id,
        Duration::from_millis(40),
    )
    .await;
    daemon
        .close_notification(receipt.notification_id)
        .await
        .unwrap();
    let events = daemon.sender_events().await;
    presenter.acknowledge_oldest();
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(daemon.sender_events().await, events);
    assert_eq!(
        daemon.snapshot().await.unwrap().history[0].status.as_str(),
        "closed"
    );
}

#[tokio::test]
async fn timeout_waits_for_presenter_acknowledgement_before_counting() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = DelayedAcknowledgementPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    let receipt = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(20),
            ..notification("Calendar", "Waiting for display")
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(40)).await;
    assert!(
        daemon
            .snapshot()
            .await
            .unwrap()
            .active
            .iter()
            .any(|thread| {
                thread.id == receipt.notification_id && thread.status.as_str() == "active"
            })
    );

    presenter.acknowledge_latest();
    tokio::time::sleep(Duration::from_millis(40)).await;

    let snapshot = daemon.snapshot().await.unwrap();
    let thread = snapshot
        .history
        .iter()
        .find(|thread| thread.id == receipt.notification_id)
        .unwrap();
    assert_eq!(thread.status.as_str(), "expired");
}

#[tokio::test]
async fn latest_queued_presentation_starts_all_visible_notification_timeouts() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = DelayedAcknowledgementPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    let first = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(20),
            ..notification("Calendar", "First queued banner")
        })
        .await
        .unwrap();
    let second = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(20),
            ..notification("Calendar", "Second queued banner")
        })
        .await
        .unwrap();

    let mut events = daemon.subscribe_sender_events();
    presenter.acknowledge_latest();
    // Model a busy executor before the newly scheduled timer tasks can run.
    std::thread::sleep(Duration::from_millis(80));
    // Expiration includes asynchronous storage commitment. A fixed sleep is not
    // evidence that both transitions have finished, even when timers are due.
    let expired = tokio::time::timeout(Duration::from_secs(2), async {
        let mut expired = std::collections::BTreeSet::new();
        for _ in 0..2 {
            let SenderEvent::NotificationClosed {
                notification_id,
                reason: canopa_notify::SenderCloseReason::Expired,
            } = events.recv().await.unwrap()
            else {
                panic!("expected a committed expiration");
            };
            expired.insert(notification_id);
        }
        expired
    })
    .await
    .expect("latest presentation must start both visible notification timeouts");
    assert_eq!(
        expired,
        [first.notification_id, second.notification_id].into()
    );

    let snapshot = daemon.snapshot().await.unwrap();
    for notification_id in [first.notification_id, second.notification_id] {
        let thread = snapshot
            .history
            .iter()
            .find(|thread| thread.id == notification_id)
            .unwrap();
        assert_eq!(thread.status.as_str(), "expired");
    }
}

#[tokio::test]
async fn a_displayed_card_expires_without_starting_a_clipped_cards_timeout() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = DelayedAcknowledgementPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();
    let lower = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(20),
            ..notification("Calendar", "Clipped lower card")
        })
        .await
        .unwrap();
    let upper = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(20),
            ..notification("Calendar", "Fully visible upper card")
        })
        .await
        .unwrap();
    let targets: Vec<_> = presenter
        .updates
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .banners
        .iter()
        .map(BannerView::target)
        .collect();
    let upper_target = *targets
        .iter()
        .find(|target| target.notification_id == upper.notification_id)
        .unwrap();
    let lower_target = *targets
        .iter()
        .find(|target| target.notification_id == lower.notification_id)
        .unwrap();
    let frame = presenter
        .acknowledgements
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    let mut events = daemon.subscribe_sender_events();
    frame.acknowledge_banner(canopa_notify::BannerTarget {
        revision: upper_target.revision + 1,
        ..upper_target
    });
    assert_active_without_sender_close_after(
        &daemon,
        upper.notification_id,
        Duration::from_millis(60),
    )
    .await;
    frame.acknowledge_banner(upper_target);
    frame.acknowledge_banner(upper_target);
    let expired = tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        expired,
        SenderEvent::NotificationClosed {
            notification_id: upper.notification_id,
            reason: canopa_notify::SenderCloseReason::Expired,
        }
    );
    assert_active_without_sender_close_after(
        &daemon,
        lower.notification_id,
        Duration::from_millis(60),
    )
    .await;
    frame.acknowledge_banner(lower_target);
    let expired = tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        expired,
        SenderEvent::NotificationClosed {
            notification_id: lower.notification_id,
            reason: canopa_notify::SenderCloseReason::Expired,
        }
    );
    frame.acknowledge_banner(lower_target);
    frame.acknowledge();
    assert_eq!(daemon.sender_events().await.len(), 2);
}

#[tokio::test]
async fn sender_requested_timeout_expires_banner_even_when_default_is_never() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(
        DaemonOptions::graphical(
            temp.path().join("history.db"),
            RecordingPresenter::default(),
        )
        .with_notification_timeout_defaults(NotificationTimeoutDefaults::new(None, None, None)),
    )
    .await
    .unwrap();

    let receipt = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(10),
            ..notification("Calendar", "Sender timeout")
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(30)).await;

    assert!(!is_visible(
        &daemon.visible_banners().await,
        receipt.notification_id
    ));
}

#[tokio::test]
async fn old_timeout_does_not_expire_replacement_revision() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(
        DaemonOptions::graphical(
            temp.path().join("history.db"),
            RecordingPresenter::default(),
        )
        .with_notification_timeout_defaults(NotificationTimeoutDefaults::new(None, None, None)),
    )
    .await
    .unwrap();

    let first = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(40),
            ..notification("Calendar", "Original")
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(10)).await;

    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(first.notification_id),
            expire_timeout: NotificationTimeout::Milliseconds(100),
            ..notification("Calendar", "Replacement")
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(is_visible(
        &daemon.visible_banners().await,
        first.notification_id
    ));
    let snapshot = daemon.snapshot().await.unwrap();
    let thread = snapshot
        .history
        .iter()
        .find(|thread| thread.id == first.notification_id)
        .unwrap();
    assert_eq!(thread.status.as_str(), "active");
    assert_eq!(thread.accepted_notification.summary, "Replacement");

    tokio::time::sleep(Duration::from_millis(70)).await;

    let snapshot = daemon.snapshot().await.unwrap();
    let thread = snapshot
        .history
        .iter()
        .find(|thread| thread.id == first.notification_id)
        .unwrap();
    assert_eq!(thread.status.as_str(), "expired");
}

#[tokio::test]
async fn overflowed_replacement_cancels_displayed_revisions_timeout_without_starting_another() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        RecordingPresenter::default(),
    ))
    .await
    .unwrap();

    let original = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(30),
            ..notification("Chat", "Displayed original")
        })
        .await
        .unwrap();
    for index in 0..3 {
        daemon
            .receive_notification(NotificationInput {
                urgency: Urgency::Critical,
                expire_timeout: NotificationTimeout::Never,
                ..notification("Alarm", format!("Critical {index}"))
            })
            .await
            .unwrap();
    }
    assert!(!is_visible(
        &daemon.visible_banners().await,
        original.notification_id
    ));

    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(original.notification_id),
            expire_timeout: NotificationTimeout::Milliseconds(20),
            ..notification("Chat", "Overflowed replacement")
        })
        .await
        .unwrap();

    let thread = assert_active_without_sender_close_after(
        &daemon,
        original.notification_id,
        Duration::from_millis(60),
    )
    .await;
    assert_eq!(thread.revision, 2);
    assert_eq!(
        thread.accepted_notification.summary,
        "Overflowed replacement"
    );
}

#[tokio::test]
async fn dropped_replacement_preserves_displayed_revisions_timeout() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        RecordingPresenter::default(),
    ))
    .await
    .unwrap();

    let original = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(40),
            ..notification("Chat", "Displayed original")
        })
        .await
        .unwrap();
    daemon
        .upsert_user_rule(UserRule::for_app("Chat", DeliveryOutcome::Dropped))
        .await
        .unwrap();
    let dropped = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(original.notification_id),
            expire_timeout: NotificationTimeout::Never,
            ..notification("Chat", "Dropped replacement")
        })
        .await
        .unwrap();
    assert_eq!(dropped.notification_id, original.notification_id);
    assert_eq!(dropped.delivery_outcome, DeliveryOutcome::Dropped);

    tokio::time::sleep(Duration::from_millis(70)).await;

    let snapshot = daemon.snapshot().await.unwrap();
    let thread = snapshot
        .history
        .iter()
        .find(|thread| thread.id == original.notification_id)
        .unwrap();
    assert_eq!(thread.status.as_str(), "expired");
    assert_eq!(thread.revision, 1);
    assert_eq!(thread.accepted_notification.summary, "Displayed original");
}

#[tokio::test]
async fn policy_change_does_not_reclassify_or_cancel_existing_timeout() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(
        DaemonOptions::graphical(
            temp.path().join("history.db"),
            RecordingPresenter::default(),
        )
        .with_notification_timeout_defaults(NotificationTimeoutDefaults::new(
            Some(Duration::from_millis(200)),
            Some(Duration::from_millis(200)),
            None,
        )),
    )
    .await
    .unwrap();

    let receipt = daemon
        .receive_notification(notification("Chat", "Visible briefly"))
        .await
        .unwrap();

    daemon
        .upsert_user_rule(UserRule::for_app("Chat", DeliveryOutcome::Silenced))
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(300)).await;

    let snapshot = daemon.snapshot().await.unwrap();
    let thread = snapshot
        .history
        .iter()
        .find(|thread| thread.id == receipt.notification_id)
        .unwrap();
    assert_eq!(thread.status.as_str(), "expired");
    assert_eq!(thread.delivery_outcome, DeliveryOutcome::Shown);
}

#[tokio::test]
async fn do_not_disturb_notification_with_finite_timeout_remains_active() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter,
    ))
    .await
    .unwrap();
    daemon.set_do_not_disturb(true).await.unwrap();

    let receipt = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(10),
            ..notification("Chat", "Silenced by DND")
        })
        .await
        .unwrap();
    assert_eq!(receipt.delivery_outcome, DeliveryOutcome::Silenced);
    assert!(!is_visible(
        &daemon.visible_banners().await,
        receipt.notification_id
    ));

    assert_active_without_sender_close_after(
        &daemon,
        receipt.notification_id,
        Duration::from_millis(30),
    )
    .await;
}

#[tokio::test]
async fn user_silenced_notification_with_finite_timeout_remains_active() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        RecordingPresenter::default(),
    ))
    .await
    .unwrap();
    daemon
        .upsert_user_rule(UserRule::for_app("Chat", DeliveryOutcome::Silenced))
        .await
        .unwrap();

    let receipt = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(10),
            ..notification("Chat", "Silenced by rule")
        })
        .await
        .unwrap();
    assert_eq!(receipt.delivery_outcome, DeliveryOutcome::Silenced);

    assert_active_without_sender_close_after(
        &daemon,
        receipt.notification_id,
        Duration::from_millis(30),
    )
    .await;
}

#[tokio::test]
async fn critical_banners_do_not_expire_by_default() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(
        DaemonOptions::graphical(
            temp.path().join("history.db"),
            RecordingPresenter::default(),
        )
        .with_notification_timeout_defaults(NotificationTimeoutDefaults::new(
            Some(Duration::from_millis(10)),
            Some(Duration::from_millis(10)),
            None,
        )),
    )
    .await
    .unwrap();

    let receipt = daemon
        .receive_notification(NotificationInput {
            urgency: Urgency::Critical,
            ..notification("Power", "Battery critical")
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(30)).await;

    assert!(is_visible(
        &daemon.visible_banners().await,
        receipt.notification_id
    ));
    let snapshot = daemon.snapshot().await.unwrap();
    let thread = snapshot
        .history
        .iter()
        .find(|thread| thread.id == receipt.notification_id)
        .unwrap();
    assert_eq!(thread.status.as_str(), "active");
}

#[tokio::test]
async fn displayed_timeout_continues_after_critical_banner_displacement() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        RecordingPresenter::default(),
    ))
    .await
    .unwrap();

    let timed = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(50),
            ..notification("Chat", "Timed normal")
        })
        .await
        .unwrap();
    for index in 0..2 {
        daemon
            .receive_notification(NotificationInput {
                expire_timeout: NotificationTimeout::Never,
                ..notification("Chat", format!("Normal {index}"))
            })
            .await
            .unwrap();
    }
    daemon
        .receive_notification(NotificationInput {
            urgency: Urgency::Critical,
            expire_timeout: NotificationTimeout::Never,
            ..notification("Alarm", "Critical displacement")
        })
        .await
        .unwrap();

    assert!(!is_visible(
        &daemon.visible_banners().await,
        timed.notification_id
    ));

    tokio::time::sleep(Duration::from_millis(80)).await;

    let snapshot = daemon.snapshot().await.unwrap();
    let thread = snapshot
        .history
        .iter()
        .find(|thread| thread.id == timed.notification_id)
        .unwrap();
    assert_eq!(thread.status.as_str(), "expired");
    assert!(daemon.sender_events().await.iter().any(|event| matches!(
        event,
        SenderEvent::NotificationClosed {
            notification_id,
            reason: canopa_notify::SenderCloseReason::Expired,
        } if *notification_id == timed.notification_id
    )));
}

#[tokio::test]
async fn acknowledged_banner_starts_timeout_even_after_daemon_state_displaces_it() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = DelayedAcknowledgementPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    let timed = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(20),
            ..notification("Chat", "Delayed display acknowledgement")
        })
        .await
        .unwrap();
    for index in 0..2 {
        daemon
            .receive_notification(NotificationInput {
                expire_timeout: NotificationTimeout::Never,
                ..notification("Chat", format!("Normal {index}"))
            })
            .await
            .unwrap();
    }
    daemon
        .receive_notification(NotificationInput {
            urgency: Urgency::Critical,
            expire_timeout: NotificationTimeout::Never,
            ..notification("Alarm", "Critical displacement")
        })
        .await
        .unwrap();
    assert!(!is_visible(
        &daemon.visible_banners().await,
        timed.notification_id
    ));

    presenter.acknowledge_oldest();
    tokio::time::sleep(Duration::from_millis(40)).await;

    let snapshot = daemon.snapshot().await.unwrap();
    let thread = snapshot
        .history
        .iter()
        .find(|thread| thread.id == timed.notification_id)
        .unwrap();
    assert_eq!(thread.status.as_str(), "expired");
}

#[tokio::test]
async fn restart_persists_threads_without_restoring_runtime_banners() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();

    let receipt = daemon
        .receive_notification(notification("Mail", "Inbox"))
        .await
        .unwrap();
    assert_eq!(daemon.visible_banners().await.len(), 1);
    drop(daemon);

    let restarted = Daemon::open(DaemonOptions::headless(database))
        .await
        .unwrap();
    let snapshot = restarted.snapshot().await.unwrap();
    let thread = snapshot
        .history
        .iter()
        .find(|thread| thread.id == receipt.notification_id)
        .unwrap();

    assert_eq!(thread.status.as_str(), "stale");
    assert!(restarted.visible_banners().await.is_empty());
}

#[tokio::test]
async fn activating_banner_invokes_default_notification_action_when_present() {
    let daemon = daemon_support::open_headless_daemon().await;

    let receipt = daemon
        .receive_notification(NotificationInput {
            actions: vec![NotificationAction::new("default", "Open")],
            ..notification("Mail", "Inbox")
        })
        .await
        .unwrap();

    daemon
        .activate_banner(receipt.notification_id)
        .await
        .unwrap();

    let events = daemon.sender_events().await;
    assert!(events.iter().any(|event| {
        matches!(
            event,
            SenderEvent::ActionInvoked {
                notification_id,
                action_key,
            } if *notification_id == receipt.notification_id && action_key == "default"
        )
    }));
}

#[tokio::test]
async fn visible_banner_keeps_every_named_action_in_sender_order() {
    let daemon = daemon_support::open_headless_daemon().await;

    daemon
        .receive_notification(NotificationInput {
            actions: vec![
                NotificationAction::new("default", "Open"),
                NotificationAction::new("reply", "Reply"),
                NotificationAction::new("archive", "Archive"),
                NotificationAction::new("mark_read", "Mark read"),
                NotificationAction::new("Fourth/α", "Fourth action"),
            ],
            ..notification("Mail", "Inbox")
        })
        .await
        .unwrap();

    let banners = daemon.visible_banners().await;
    assert_eq!(
        banners[0]
            .actions
            .iter()
            .map(|action| (action.key.as_str(), action.label.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("reply", "Reply"),
            ("archive", "Archive"),
            ("mark_read", "Mark read"),
            ("Fourth/α", "Fourth action"),
        ]
    );
    assert_eq!(banners[0].default_action_key.as_deref(), Some("default"));
}

#[tokio::test]
async fn activating_banner_without_default_action_dismisses_notification() {
    let daemon = daemon_support::open_headless_daemon().await;

    let receipt = daemon
        .receive_notification(notification("Calendar", "Standup"))
        .await
        .unwrap();

    daemon
        .activate_banner(receipt.notification_id)
        .await
        .unwrap();

    let snapshot = daemon.snapshot().await.unwrap();
    let thread = snapshot
        .history
        .iter()
        .find(|thread| thread.id == receipt.notification_id)
        .unwrap();
    assert_eq!(thread.status.as_str(), "dismissed");
}

#[tokio::test]
async fn banner_visibility_is_capped_and_hidden_notifications_are_not_promoted_later() {
    let daemon = daemon_support::open_headless_daemon().await;

    let mut ids = Vec::new();
    for index in 0..4 {
        ids.push(
            daemon
                .receive_notification(notification("Chat", format!("Message {index}")))
                .await
                .unwrap()
                .notification_id,
        );
    }

    assert_eq!(visible_ids(&daemon.visible_banners().await).len(), 3);
    assert!(is_visible(&daemon.visible_banners().await, ids[3]));
    assert!(!is_visible(&daemon.visible_banners().await, ids[0]));

    daemon.dismiss_notification(ids[3]).await.unwrap();
    assert!(!is_visible(&daemon.visible_banners().await, ids[0]));
}

#[tokio::test]
async fn presentation_reports_overflow_count_without_expanding_visible_banner_set() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    for index in 0..5 {
        daemon
            .receive_notification(notification("Chat", format!("Message {index}")))
            .await
            .unwrap();
    }

    let updates = presenter.presentation_updates();
    let presentation = updates.last().unwrap();
    assert_eq!(presentation.banners.len(), 3);
    assert_eq!(presentation.overflow_count, 2);

    let dismissed = presentation.banners[0].notification_id;
    daemon.dismiss_notification(dismissed).await.unwrap();

    let updates = presenter.presentation_updates();
    let presentation = updates.last().unwrap();
    assert_eq!(presentation.banners.len(), 2);
    assert_eq!(presentation.overflow_count, 2);
}

#[tokio::test]
async fn critical_notifications_displace_normal_banners_when_slots_are_full() {
    let daemon = daemon_support::open_headless_daemon().await;

    for index in 0..3 {
        daemon
            .receive_notification(notification("Chat", format!("Normal {index}")))
            .await
            .unwrap();
    }

    let critical = daemon
        .receive_notification(NotificationInput {
            urgency: Urgency::Critical,
            ..notification("Alarm", "Battery critical")
        })
        .await
        .unwrap();

    let banners = daemon.visible_banners().await;
    assert_eq!(visible_ids(&banners).len(), 3);
    assert!(is_visible(&banners, critical.notification_id));
    assert_eq!(
        banners
            .iter()
            .filter(|banner| banner.urgency == Urgency::Normal)
            .count(),
        2
    );
}

#[tokio::test]
async fn fourth_normal_notification_behind_critical_banners_does_not_start_timeout() {
    let temp = tempfile::tempdir().unwrap();
    let presenter = RecordingPresenter::default();
    let daemon = Daemon::open(DaemonOptions::graphical(
        temp.path().join("history.db"),
        presenter.clone(),
    ))
    .await
    .unwrap();

    let mut critical_ids = Vec::new();
    for index in 0..3 {
        critical_ids.push(
            daemon
                .receive_notification(NotificationInput {
                    urgency: Urgency::Critical,
                    expire_timeout: NotificationTimeout::Never,
                    ..notification("Alarm", format!("Critical {index}"))
                })
                .await
                .unwrap()
                .notification_id,
        );
    }

    let overflowed = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(10),
            ..notification("Chat", "Overflowed normal")
        })
        .await
        .unwrap();

    let presentation = presenter.presentation_updates().pop().unwrap();
    assert_eq!(presentation.banners.len(), 3);
    assert_eq!(presentation.overflow_count, 1);
    assert!(!is_visible(
        &presentation.banners,
        overflowed.notification_id
    ));

    daemon.dismiss_notification(critical_ids[0]).await.unwrap();
    assert!(!is_visible(
        &daemon.visible_banners().await,
        overflowed.notification_id
    ));

    assert_active_without_sender_close_after(
        &daemon,
        overflowed.notification_id,
        Duration::from_millis(30),
    )
    .await;
}

async fn assert_active_without_sender_close_after(
    daemon: &Daemon,
    notification_id: canopa_notify::InternalNotificationId,
    delay: Duration,
) -> NotificationThread {
    tokio::time::sleep(delay).await;

    let thread = daemon
        .snapshot()
        .await
        .unwrap()
        .history
        .into_iter()
        .find(|thread| thread.id == notification_id)
        .unwrap();
    assert_eq!(thread.status.as_str(), "active");
    assert!(!daemon.sender_events().await.iter().any(|event| matches!(
        event,
        SenderEvent::NotificationClosed {
            notification_id: closed_id,
            ..
        } if *closed_id == notification_id
    )));
    thread
}

fn visible_ids(banners: &[BannerView]) -> Vec<canopa_notify::InternalNotificationId> {
    banners
        .iter()
        .map(|banner| banner.notification_id)
        .collect()
}

fn is_visible(banners: &[BannerView], id: canopa_notify::InternalNotificationId) -> bool {
    banners.iter().any(|banner| banner.notification_id == id)
}

fn test_image_data() -> NotificationImageData {
    NotificationImageData::from_freedesktop(1, 1, 4, true, 8, 4, vec![255, 0, 0, 255]).unwrap()
}

fn test_png_bytes() -> &'static [u8] {
    &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240,
        31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ]
}

fn path_string(path: &std::path::Path) -> String {
    path.to_str().unwrap().to_string()
}
