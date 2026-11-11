use crate::notifications;

use canopa_notify::{
    Daemon, DaemonOptions, DeliveryOutcome, NoopBannerPresenter, NotificationInput,
    NotificationSound, NotificationSoundPlayback, NotificationSoundSettings, SoundPresenter,
    Urgency,
};
use notifications::notification;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

struct GraphicalDaemon {
    _temp: tempfile::TempDir,
    daemon: Daemon,
}

impl Deref for GraphicalDaemon {
    type Target = Daemon;

    fn deref(&self) -> &Self::Target {
        &self.daemon
    }
}

#[derive(Clone, Default)]
struct RecordingSoundPresenter {
    state: Arc<Mutex<RecordingSoundState>>,
}

#[derive(Default)]
struct RecordingSoundState {
    settings: Vec<NotificationSoundSettings>,
    playback: Vec<NotificationSoundPlayback>,
    stops: usize,
    available: bool,
}

impl RecordingSoundPresenter {
    fn available() -> Self {
        let presenter = Self::default();
        presenter.state.lock().unwrap().available = true;
        presenter
    }

    fn playback(&self) -> Vec<NotificationSoundPlayback> {
        self.state.lock().unwrap().playback.clone()
    }

    fn settings(&self) -> Vec<NotificationSoundSettings> {
        self.state.lock().unwrap().settings.clone()
    }

    fn stops(&self) -> usize {
        self.state.lock().unwrap().stops
    }
}

impl SoundPresenter for RecordingSoundPresenter {
    fn apply_sound_settings(&self, settings: NotificationSoundSettings) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.available = settings.enabled;
        state.settings.push(settings);
        Ok(())
    }

    fn play_notification_sound(&self, playback: NotificationSoundPlayback) -> anyhow::Result<()> {
        self.state.lock().unwrap().playback.push(playback);
        Ok(())
    }

    fn stop_all_sounds(&self) {
        self.state.lock().unwrap().stops += 1;
    }

    fn supports_sound(&self) -> bool {
        self.state.lock().unwrap().available
    }
}

#[tokio::test]
async fn failed_runtime_configuration_install_preserves_the_active_configuration() {
    struct RejectSettings;
    impl SoundPresenter for RejectSettings {
        fn apply_sound_settings(&self, settings: NotificationSoundSettings) -> anyhow::Result<()> {
            anyhow::ensure!(settings.theme != "reject", "injected settings failure");
            Ok(())
        }
        fn play_notification_sound(&self, _: NotificationSoundPlayback) -> anyhow::Result<()> {
            Ok(())
        }
        fn stop_all_sounds(&self) {}
        fn supports_sound(&self) -> bool {
            false
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(
        DaemonOptions::graphical(temp.path().join("history.db"), NoopBannerPresenter)
            .with_sound_presenter(RejectSettings),
    )
    .await
    .unwrap();
    let config = temp.path().join("config.kdl");
    std::fs::write(&config, "rule app=\"Mail\" outcome=\"silence\"\n").unwrap();
    daemon.reload_configuration_file(&config).await.unwrap();
    std::fs::write(
        &config,
        "rule app=\"Mail\" outcome=\"drop\"\nsound theme=\"reject\"\n",
    )
    .unwrap();
    assert!(daemon.reload_configuration_file(&config).await.is_err());
    assert!(
        daemon.configuration_error().await.is_some(),
        "runtime install failure was not recorded"
    );
    let receipt = daemon
        .receive_notification(notification("Mail", "Must still be silenced"))
        .await
        .unwrap();
    assert_eq!(receipt.delivery_outcome, DeliveryOutcome::Silenced);
}

#[tokio::test]
async fn shown_notification_plays_default_sound_once() {
    let sound = RecordingSoundPresenter::available();
    let daemon = open_graphical_daemon(sound.clone()).await;

    let receipt = daemon
        .receive_notification(notification("Chat", "Message"))
        .await
        .unwrap();

    let playback = sound.playback();
    assert_eq!(playback.len(), 1);
    assert_eq!(playback[0].notification_id, receipt.notification_id);
    assert_eq!(playback[0].app_identity, "Chat");
    assert_eq!(playback[0].summary, "Message");
    assert_eq!(playback[0].requested_sound, None);
}

#[tokio::test]
async fn transient_notification_plays_sound_without_entering_history() {
    let sound = RecordingSoundPresenter::available();
    let daemon = open_graphical_daemon(sound.clone()).await;

    let receipt = daemon
        .receive_notification(NotificationInput {
            transient: true,
            ..notification("Chat", "Typing")
        })
        .await
        .unwrap();

    assert_eq!(sound.playback()[0].notification_id, receipt.notification_id);
    assert!(daemon.snapshot().await.unwrap().history.is_empty());
}

#[tokio::test]
async fn suppress_sound_prevents_default_and_requested_sound() {
    let sound = RecordingSoundPresenter::available();
    let daemon = open_graphical_daemon(sound.clone()).await;

    daemon
        .receive_notification(NotificationInput {
            suppress_sound: true,
            sound: NotificationSound::name("message-new-instant"),
            ..notification("Chat", "Message")
        })
        .await
        .unwrap();

    assert!(sound.playback().is_empty());
}

#[tokio::test]
async fn silenced_notification_does_not_play_sound_but_critical_dnd_bypass_does() {
    let sound = RecordingSoundPresenter::available();
    let daemon = open_graphical_daemon(sound.clone()).await;
    daemon.set_do_not_disturb(true).await.unwrap();

    let silenced = daemon
        .receive_notification(notification("Chat", "Quiet"))
        .await
        .unwrap();
    let critical = daemon
        .receive_notification(NotificationInput {
            urgency: Urgency::Critical,
            ..notification("Monitor", "Pager")
        })
        .await
        .unwrap();

    assert_eq!(silenced.delivery_outcome, DeliveryOutcome::Silenced);
    assert_eq!(critical.delivery_outcome, DeliveryOutcome::Shown);
    assert_eq!(sound.playback().len(), 1);
    assert_eq!(
        sound.playback()[0].notification_id,
        critical.notification_id
    );
}

#[tokio::test]
async fn shown_replacement_plays_sound_again() {
    let sound = RecordingSoundPresenter::available();
    let daemon = open_graphical_daemon(sound.clone()).await;

    let first = daemon
        .receive_notification(notification("Downloader", "Download"))
        .await
        .unwrap();
    let replacement = daemon
        .receive_notification(NotificationInput {
            replaces_id: Some(first.notification_id),
            sound: NotificationSound::file("/tmp/complete.oga"),
            ..notification("Downloader", "Download")
        })
        .await
        .unwrap();

    assert_eq!(replacement.notification_id, first.notification_id);
    let playback = sound.playback();
    assert_eq!(playback.len(), 2);
    assert_eq!(
        playback[1].requested_sound,
        Some(NotificationSound::File("/tmp/complete.oga".to_string()))
    );
}

#[tokio::test]
async fn sound_only_notification_is_noop_and_does_not_play() {
    let sound = RecordingSoundPresenter::available();
    let daemon = open_graphical_daemon(sound.clone()).await;

    let receipt = daemon
        .receive_notification(NotificationInput {
            sound: NotificationSound::name("message-new-instant"),
            ..NotificationInput::default()
        })
        .await
        .unwrap();

    assert_ne!(receipt.notification_id.get(), 0);
    assert_eq!(receipt.delivery_outcome, DeliveryOutcome::Dropped);
    assert!(sound.playback().is_empty());
}

#[tokio::test]
async fn sound_configuration_reload_changes_capability_for_future_notifications() {
    let sound = RecordingSoundPresenter::available();
    let daemon = open_graphical_daemon(sound.clone()).await;
    assert!(daemon.supports_sound());
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("notify.kdl");
    std::fs::write(&config, r#"sound enabled=#false theme="freedesktop""#).unwrap();

    daemon.reload_configuration_file(&config).await.unwrap();
    daemon
        .receive_notification(notification("Chat", "Quiet"))
        .await
        .unwrap();

    assert!(!daemon.supports_sound());
    assert_eq!(sound.settings().len(), 1);
    assert!(!sound.settings()[0].enabled);
    assert_eq!(sound.stops(), 1);
    assert!(sound.playback().is_empty());
}

#[tokio::test]
async fn headless_daemon_ignores_sound_presenter_and_config() {
    let sound = RecordingSoundPresenter::available();
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db")).with_sound_presenter(sound.clone()),
    )
    .await
    .unwrap();
    let config = temp.path().join("notify.kdl");
    std::fs::write(&config, r#"sound enabled=#true"#).unwrap();

    daemon.reload_configuration_file(&config).await.unwrap();
    daemon
        .receive_notification(notification("Chat", "Quiet"))
        .await
        .unwrap();

    assert!(!daemon.supports_sound());
    assert!(sound.settings().is_empty());
    assert!(sound.playback().is_empty());
}

async fn open_graphical_daemon(sound: RecordingSoundPresenter) -> GraphicalDaemon {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let daemon = Daemon::open(
        DaemonOptions::graphical(database, NoopBannerPresenter).with_sound_presenter(sound),
    )
    .await
    .unwrap();
    GraphicalDaemon {
        _temp: temp,
        daemon,
    }
}

#[tokio::test]
async fn app_sound_preference_mutes_only_that_application_and_persists() {
    let sound = RecordingSoundPresenter::available();
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let open = |sound: RecordingSoundPresenter| {
        Daemon::open(
            DaemonOptions::graphical(database.clone(), NoopBannerPresenter)
                .with_sound_presenter(sound),
        )
    };
    let daemon = open(sound.clone()).await.unwrap();
    daemon
        .set_app_preferences(
            "Chat",
            canopa_notify::AppPreferences {
                sound: Some(false),
                lock_screen: None,
            },
        )
        .await
        .unwrap();
    let muted = daemon
        .receive_notification(notification("Chat", "Quiet"))
        .await
        .unwrap();
    assert_eq!(
        muted.delivery_outcome,
        DeliveryOutcome::Shown,
        "banner still shown"
    );
    daemon
        .receive_notification(notification("Mail", "Loud"))
        .await
        .unwrap();
    assert_eq!(
        sound
            .playback()
            .iter()
            .map(|p| p.app_identity.as_str())
            .collect::<Vec<_>>(),
        ["Mail"]
    );
    daemon.shutdown().await.unwrap();
    drop(daemon);

    let restarted_sound = RecordingSoundPresenter::available();
    let restarted = open(restarted_sound.clone()).await.unwrap();
    restarted
        .receive_notification(notification("Chat", "Still quiet"))
        .await
        .unwrap();
    assert!(restarted_sound.playback().is_empty());
}
