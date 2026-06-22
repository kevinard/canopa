#[path = "support/daemon.rs"]
mod daemon_support;
#[path = "support/notifications.rs"]
mod notifications;

use canopa_notify::{
    BannerKeyboardInteractivity, BannerLayer, BannerOutputSelection, BannerPresentation,
    BannerPresenter, BannerSurfaceSpec, BannerView, Daemon, DaemonOptions, DeliveryOutcome,
    IcedLayerShellBannerPresenter, NotificationAction, NotificationImageData, NotificationInput,
    NotificationTimeout, NotificationTimeoutDefaults, NotificationVisualReference, Urgency,
    UserRule,
};
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
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

#[test]
fn notification_banner_surface_maps_to_iced_layer_shell_settings() {
    let settings = IcedLayerShellBannerPresenter::layer_shell_settings(
        BannerSurfaceSpec::notification_banner(),
    );

    assert_eq!(settings.anchor, Anchor::Top | Anchor::Right);
    assert_eq!(settings.layer, Layer::Overlay);
    assert_eq!(settings.keyboard_interactivity, KeyboardInteractivity::None);
    assert!(matches!(settings.start_mode, StartMode::Active));
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
    assert!(banners[0].body_markup.runs[0].style.bold);
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
async fn visible_normal_banners_expire_after_configured_timeout() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db")).with_notification_timeout_defaults(
            NotificationTimeoutDefaults::new(
                Some(Duration::from_millis(10)),
                Some(Duration::from_millis(10)),
                None,
            ),
        ),
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
async fn sender_requested_timeout_expires_banner_even_when_default_is_never() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db"))
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
        DaemonOptions::headless(temp.path().join("history.db"))
            .with_notification_timeout_defaults(NotificationTimeoutDefaults::new(None, None, None)),
    )
    .await
    .unwrap();

    let first = daemon
        .receive_notification(NotificationInput {
            expire_timeout: NotificationTimeout::Milliseconds(10),
            ..notification("Calendar", "Original")
        })
        .await
        .unwrap();

    daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(first.notification_id),
            expire_timeout: NotificationTimeout::Never,
            ..notification("Calendar", "Replacement")
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(30)).await;

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
}

#[tokio::test]
async fn silencing_active_notification_cancels_pending_timeout() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db")).with_notification_timeout_defaults(
            NotificationTimeoutDefaults::new(
                Some(Duration::from_millis(200)),
                Some(Duration::from_millis(200)),
                None,
            ),
        ),
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
    assert_eq!(thread.status.as_str(), "active");
    assert_eq!(thread.delivery_outcome, DeliveryOutcome::Silenced);
}

#[tokio::test]
async fn critical_banners_do_not_expire_by_default() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db")).with_notification_timeout_defaults(
            NotificationTimeoutDefaults::new(
                Some(Duration::from_millis(10)),
                Some(Duration::from_millis(10)),
                None,
            ),
        ),
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
        event.notification_id == receipt.notification_id
            && event.action_key.as_deref() == Some("default")
    }));
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

fn visible_ids(banners: &[BannerView]) -> Vec<i64> {
    banners
        .iter()
        .map(|banner| banner.notification_id)
        .collect()
}

fn is_visible(banners: &[BannerView], id: i64) -> bool {
    banners.iter().any(|banner| banner.notification_id == id)
}

fn test_image_data() -> NotificationImageData {
    NotificationImageData::from_freedesktop(1, 1, 4, true, 8, 4, vec![255, 0, 0, 255]).unwrap()
}

fn path_string(path: &std::path::Path) -> String {
    path.to_str().unwrap().to_string()
}
