use super::*;

fn offline_presenter() -> (RodioSoundPresenter, rodio::mixer::MixerSource) {
    offline_presenter_with_dirs(Vec::new())
}

fn offline_presenter_with_dirs(
    data_dirs: Vec<PathBuf>,
) -> (RodioSoundPresenter, rodio::mixer::MixerSource) {
    offline_presenter_with_format(data_dirs, 1, 8_000)
}

fn offline_presenter_with_format(
    data_dirs: Vec<PathBuf>,
    channels: u16,
    rate: u32,
) -> (RodioSoundPresenter, rodio::mixer::MixerSource) {
    offline_presenter_with_resolver(SoundThemeResolver::new(data_dirs, "C"), channels, rate)
}

fn offline_presenter_with_resolver(
    resolver: SoundThemeResolver,
    channels: u16,
    rate: u32,
) -> (RodioSoundPresenter, rodio::mixer::MixerSource) {
    let (mixer, output) =
        rodio::mixer::mixer(channels.try_into().unwrap(), rate.try_into().unwrap());
    let presenter = RodioSoundPresenter::with_backend_opener(
        NotificationSoundSettings::default(),
        resolver,
        Arc::new(move || {
            Ok(RodioBackend {
                _sink: None,
                mixer: mixer.clone(),
            })
        }),
    );
    (presenter, output)
}

fn wave_file(path: &Path, sample_count: u32) {
    wave_file_with_format(path, sample_count, 1, 8_000);
}

fn wave_file_with_format(path: &Path, sample_count: u32, channels: u16, rate: u32) {
    let data_size = sample_count * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0");
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(b"\x10\0data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    for _ in 0..sample_count {
        bytes.extend_from_slice(&8_192_i16.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

fn file_playback(path: &Path) -> NotificationSoundPlayback {
    NotificationSoundPlayback {
        notification_id: 1.try_into().unwrap(),
        app_identity: "test".to_string(),
        summary: "test sound".to_string(),
        requested_sound: Some(NotificationSound::File(path.to_str().unwrap().to_string())),
    }
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(
            std::time::Instant::now() < deadline,
            "offline sound operation timed out"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

mod theme;

#[test]
fn sound_presentation_admits_at_most_four_jobs_including_unconsumed_audio() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sound.wav");
    wave_file(&path, 8_000);
    let (presenter, _output) = offline_presenter();

    for _ in 0..4 {
        presenter
            .play_notification_sound(file_playback(&path))
            .unwrap();
    }
    assert_eq!(
        presenter
            .play_notification_sound(file_playback(&path))
            .unwrap_err()
            .to_string(),
        "notification sound capacity exhausted"
    );
}

#[test]
fn disabling_sound_cancels_playback_without_resetting_outstanding_capacity() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sound.wav");
    wave_file(&path, 8_000);
    let (presenter, mut output) = offline_presenter();
    for _ in 0..4 {
        presenter
            .play_notification_sound(file_playback(&path))
            .unwrap();
    }
    // Each voice is exactly 0.25. Wait for all four sources, not merely
    // the first worker: cancellation may legitimately release decoding jobs.
    wait_until(|| output.next() == Some(1.0));

    presenter
        .apply_sound_settings(NotificationSoundSettings {
            enabled: false,
            ..Default::default()
        })
        .unwrap();
    assert!(!presenter.supports_sound());
    presenter
        .apply_sound_settings(NotificationSoundSettings::default())
        .unwrap();
    assert!(presenter.supports_sound());
    // An offline output deliberately retains old sources after closing the
    // device: reopening must not forget their still-occupied reservations.
    assert!(
        presenter
            .play_notification_sound(file_playback(&path))
            .is_err()
    );
    assert_eq!(output.next(), None, "disabled audio continued playing");
    wait_until(|| {
        assert_eq!(output.next(), None, "late cancelled audio became audible");
        presenter
            .play_notification_sound(file_playback(&path))
            .is_ok()
    });
    wait_until(|| output.next().is_some());
}

#[test]
fn sound_presentation_truncates_audio_at_ten_seconds() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("long.wav");
    wave_file(&path, 80_008);
    let (presenter, mut output) = offline_presenter();
    presenter
        .play_notification_sound(file_playback(&path))
        .unwrap();
    wait_until(|| output.next().is_some());
    assert_eq!(1 + output.by_ref().count(), 80_000);
}

#[test]
fn oversized_sound_file_uses_default_fallback() {
    let temp = tempfile::tempdir().unwrap();
    let theme = temp.path().join("sounds/freedesktop/stereo");
    std::fs::create_dir_all(&theme).unwrap();
    wave_file(&theme.join("message-new-instant.wav"), 8);
    let path = temp.path().join("oversized.wav");
    wave_file(&path, 8_000);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(8 * 1024 * 1024 + 1)
        .unwrap();
    let (presenter, mut output) = offline_presenter_with_dirs(vec![temp.path().to_path_buf()]);

    presenter
        .play_notification_sound(file_playback(&path))
        .unwrap();
    wait_until(|| output.next().is_some());
    assert_eq!(
        1 + output.by_ref().count(),
        8,
        "oversized request played instead of default"
    );
}

#[test]
fn sound_presentation_truncates_pcm_at_four_mebibytes_before_duration_limit() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("large-pcm.wav");
    wave_file_with_format(&path, 1_048_584, 2, 192_000);
    let (presenter, mut output) = offline_presenter_with_format(Vec::new(), 2, 192_000);
    presenter
        .play_notification_sound(file_playback(&path))
        .unwrap();
    wait_until(|| output.next().is_some());
    assert_eq!(1 + output.by_ref().count(), 1_048_576);
}

#[test]
fn sound_file_at_exact_byte_limit_is_played() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("exact.wav");
    wave_file(&path, 8);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(8 * 1024 * 1024)
        .unwrap();
    let (presenter, mut output) = offline_presenter();
    presenter
        .play_notification_sound(file_playback(&path))
        .unwrap();
    wait_until(|| output.next().is_some());
    assert_eq!(1 + output.by_ref().count(), 8);
}

#[test]
fn fifo_and_invalid_sound_files_fall_back_without_blocking() {
    let temp = tempfile::tempdir().unwrap();
    let theme = temp.path().join("sounds/freedesktop/stereo");
    std::fs::create_dir_all(&theme).unwrap();
    wave_file(&theme.join("message-new-instant.wav"), 8);
    let fifo = temp.path().join("sound.fifo");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let invalid = temp.path().join("invalid.wav");
    std::fs::write(&invalid, b"not audio").unwrap();
    let missing = temp.path().join("missing.wav");
    let unsupported = temp.path().join("unsupported.wav");
    wave_file_with_format(&unsupported, 8, 1, 192_001);
    for path in [&fifo, &invalid, &missing, &unsupported, &theme] {
        let (presenter, mut output) = offline_presenter_with_dirs(vec![temp.path().to_path_buf()]);
        presenter
            .play_notification_sound(file_playback(path))
            .unwrap();
        wait_until(|| output.next().is_some());
        assert_eq!(1 + output.by_ref().count(), 8);
    }
}

#[test]
fn simultaneous_sound_requests_share_the_same_admission_limit() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sound.wav");
    wave_file(&path, 8_000);
    let (presenter, _output) = offline_presenter();
    let barrier = std::sync::Barrier::new(32);
    let admitted = std::thread::scope(|scope| {
        let handles = (0..32)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    presenter
                        .play_notification_sound(file_playback(&path))
                        .is_ok()
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(|admitted| *admitted)
            .count()
    });
    assert_eq!(admitted, 4);
}

#[test]
fn stop_and_changed_settings_cancel_audio_but_identical_settings_do_not() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sound.wav");
    wave_file(&path, 8_000);
    for stop in [false, true] {
        let (presenter, mut output) = offline_presenter();
        presenter
            .play_notification_sound(file_playback(&path))
            .unwrap();
        wait_until(|| output.next().is_some());
        presenter
            .apply_sound_settings(NotificationSoundSettings::default())
            .unwrap();
        assert!(
            output.next().is_some(),
            "identical settings cancelled audio"
        );
        if stop {
            presenter.stop_all_sounds();
            assert!(!presenter.supports_sound());
        } else {
            presenter
                .apply_sound_settings(NotificationSoundSettings {
                    theme: "custom".into(),
                    ..Default::default()
                })
                .unwrap();
            assert!(presenter.supports_sound());
        }
        assert_eq!(output.next(), None);
        presenter
            .apply_sound_settings(NotificationSoundSettings::default())
            .unwrap();
        assert!(presenter.supports_sound());
        presenter
            .play_notification_sound(file_playback(&path))
            .unwrap();
        wait_until(|| output.next().is_some());
    }
}

#[test]
fn completed_and_failed_sounds_release_capacity_during_repeated_playback() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sound.wav");
    wave_file(&path, 8);
    let (presenter, mut output) = offline_presenter();
    for _ in 0..32 {
        // Invalid requested and missing default sounds must release their job.
        let missing = temp.path().join("missing.wav");
        wait_until(|| {
            presenter
                .play_notification_sound(file_playback(&missing))
                .is_ok()
        });
        wait_until(|| {
            presenter
                .play_notification_sound(file_playback(&path))
                .is_ok()
        });
        wait_until(|| output.next().is_some());
        assert_eq!(1 + output.by_ref().count(), 8);
    }
}

#[test]
fn backend_unavailability_is_degraded_support_and_can_be_retried() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let (mixer, _output) = rodio::mixer::mixer(1.try_into().unwrap(), 8_000.try_into().unwrap());
    let presenter = RodioSoundPresenter::with_backend_opener(
        NotificationSoundSettings::default(),
        SoundThemeResolver::new(Vec::new(), "C"),
        Arc::new(move || {
            if attempts.fetch_add(1, Ordering::Relaxed) == 0 {
                anyhow::bail!("injected unavailable output");
            }
            Ok(RodioBackend {
                _sink: None,
                mixer: mixer.clone(),
            })
        }),
    );
    assert!(!presenter.supports_sound());
    presenter
        .play_notification_sound(file_playback(Path::new("/missing.wav")))
        .unwrap();
    presenter
        .apply_sound_settings(NotificationSoundSettings::default())
        .unwrap();
    assert!(presenter.supports_sound());
}

#[tokio::test]
async fn saturated_sound_preserves_notification_receipts_history_and_closures() {
    use crate::{
        Daemon, DaemonOptions, DeliveryOutcome, NoopBannerPresenter, NotificationInput,
        NotificationStatus, NotificationTimeout,
    };

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sound.wav");
    wave_file(&path, 8_000);
    let (presenter, _output) = offline_presenter();
    let daemon = Daemon::open(
        DaemonOptions::graphical(temp.path().join("history.db"), NoopBannerPresenter)
            .with_sound_presenter(presenter.clone()),
    )
    .await
    .unwrap();
    let mut receipts = Vec::new();
    for index in 0..32 {
        let receipt = daemon
            .receive_notification(NotificationInput {
                summary: "Sound pressure".into(),
                transient: index % 2 == 0,
                sound: file_playback(&path).requested_sound,
                expire_timeout: NotificationTimeout::Never,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(receipt.delivery_outcome, DeliveryOutcome::Shown);
        receipts.push(receipt);
    }
    assert!(
        presenter
            .play_notification_sound(file_playback(&path))
            .is_err()
    );
    assert!(
        daemon.supports_sound(),
        "saturation is not a capability change"
    );
    assert_eq!(daemon.snapshot().await.unwrap().history.len(), 16);
    assert!(daemon.sender_events().await.is_empty());
    for receipt in receipts {
        daemon
            .close_notification(receipt.notification_id)
            .await
            .unwrap();
    }
    assert_eq!(daemon.sender_events().await.len(), 32);
    let history = daemon.snapshot().await.unwrap().history;
    assert_eq!(history.len(), 16);
    assert!(
        history
            .iter()
            .all(|thread| thread.status == NotificationStatus::Closed)
    );
}
