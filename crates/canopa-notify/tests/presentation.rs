use canopa_notify::{
    BannerKeyboardInteractivity, BannerLayer, BannerOutputSelection, BannerPresenter,
    BannerSurfaceSpec, BannerView, Daemon, DaemonOptions, IcedLayerShellBannerPresenter,
    NotificationAction, NotificationInput, NotificationSnapshot, Urgency,
};
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::StartMode;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct RecordingPresenter {
    updates: Arc<Mutex<Vec<Vec<BannerView>>>>,
}

impl RecordingPresenter {
    fn updates(&self) -> Vec<Vec<BannerView>> {
        self.updates.lock().unwrap().clone()
    }
}

impl BannerPresenter for RecordingPresenter {
    fn replace_visible_banners(&self, banners: Vec<BannerView>) -> anyhow::Result<()> {
        self.updates.lock().unwrap().push(banners);
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
        .receive_notification(NotificationInput {
            app_name: "Example Mail".to_string(),
            summary: "Inbox".to_string(),
            body: "<b>One</b><br/>unread message".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    let updates = presenter.updates();
    let banners = updates.last().unwrap();
    assert_eq!(banners.len(), 1);
    assert_eq!(banners[0].notification_id, receipt.notification_id);
    assert_eq!(banners[0].summary, "Inbox");
    assert_eq!(banners[0].body, "One\nunread message");
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
        .receive_notification(NotificationInput {
            app_name: "Chat".to_string(),
            summary: "Stored but not rendered".to_string(),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    assert!(daemon.snapshot().await.unwrap().active[0].visible_banner);
    assert!(presenter.updates().is_empty());
}

#[tokio::test]
async fn activating_banner_invokes_default_notification_action_when_present() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();

    let receipt = daemon
        .receive_notification(NotificationInput {
            app_name: "Mail".to_string(),
            summary: "Inbox".to_string(),
            actions: vec![NotificationAction::new("default", "Open")],
            ..NotificationInput::default()
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
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();

    let receipt = daemon
        .receive_notification(NotificationInput {
            app_name: "Calendar".to_string(),
            summary: "Standup".to_string(),
            ..NotificationInput::default()
        })
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
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();

    let mut ids = Vec::new();
    for index in 0..4 {
        ids.push(
            daemon
                .receive_notification(NotificationInput {
                    app_name: "Chat".to_string(),
                    summary: format!("Message {index}"),
                    ..NotificationInput::default()
                })
                .await
                .unwrap()
                .notification_id,
        );
    }

    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(visible_ids(&snapshot).len(), 3);
    assert!(!is_visible(&snapshot, ids[3]));

    daemon.dismiss_notification(ids[0]).await.unwrap();
    let snapshot = daemon.snapshot().await.unwrap();
    assert!(!is_visible(&snapshot, ids[3]));
}

#[tokio::test]
async fn critical_notifications_displace_normal_banners_when_slots_are_full() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();

    for index in 0..3 {
        daemon
            .receive_notification(NotificationInput {
                app_name: "Chat".to_string(),
                summary: format!("Normal {index}"),
                ..NotificationInput::default()
            })
            .await
            .unwrap();
    }

    let critical = daemon
        .receive_notification(NotificationInput {
            app_name: "Alarm".to_string(),
            summary: "Battery critical".to_string(),
            urgency: Urgency::Critical,
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(visible_ids(&snapshot).len(), 3);
    assert!(is_visible(&snapshot, critical.notification_id));
    assert_eq!(
        snapshot
            .active
            .iter()
            .filter(|thread| thread.visible_banner && thread.urgency == Urgency::Normal)
            .count(),
        2
    );
}

fn visible_ids(snapshot: &NotificationSnapshot) -> Vec<i64> {
    snapshot
        .active
        .iter()
        .filter(|thread| thread.visible_banner)
        .map(|thread| thread.id)
        .collect()
}

fn is_visible(snapshot: &NotificationSnapshot, id: i64) -> bool {
    snapshot
        .active
        .iter()
        .find(|thread| thread.id == id)
        .map(|thread| thread.visible_banner)
        .unwrap_or(false)
}
