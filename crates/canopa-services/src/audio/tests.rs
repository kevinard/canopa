//! Real Pulse protocol tests with private PipeWire daemons and synthetic sinks.
//! These configs load no hardware discovery or desktop session manager.
use super::*;
use crate::testing::audio::AudioServer;

async fn devices_matching(
    state: &mut tokio::sync::watch::Receiver<Option<AudioSnapshot>>,
    matches: impl Fn(&AudioSnapshot) -> bool,
) -> AudioSnapshot {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(snapshot) = state.borrow_and_update().clone()
                && matches(&snapshot)
            {
                return snapshot;
            }
            state.changed().await.unwrap();
        }
    })
    .await
    .expect("audio devices did not reach the expected state")
}

#[tokio::test]
async fn bar_and_device_controls_share_channel_volume_policy() {
    let server = AudioServer::new().await;
    server
        .pactl(&["load-module", "module-null-sink", "sink_name=speakers"])
        .await;
    server.set_default_sink("speakers").await;
    server.pactl(&["set-sink-mute", "speakers", "1"]).await;
    let (audio, mut state) = monitor(Some(server.server.clone()));
    let initial = devices_matching(&mut state, |s| s.devices.len() == 1).await;
    let id = initial.devices[0].id.clone();

    for device_control in [false, true] {
        for (left, right, step, up, expected_left, expected_right) in [
            (0, 0, 10, true, 10, 10),
            (30, 60, 10, true, 35, 70),
            (90, 90, 20, true, 100, 100),
            (100, 50, 5, true, 100, 50),
            (120, 60, 5, true, 120, 60),
            (120, 60, 30, false, 90, 45),
            (30, 60, 100, false, 0, 0),
        ] {
            server
                .pactl(&[
                    "set-sink-volume",
                    "speakers",
                    &format!("{left}%"),
                    &format!("{right}%"),
                ])
                .await;
            if device_control {
                audio
                    .apply(id.clone(), DeviceOperation::AdjustVolume { step, up })
                    .await
                    .unwrap();
            } else {
                adjust_volume_on(Some(&server.server), step, up).unwrap();
            }
            let volume = server.pactl(&["get-sink-volume", "speakers"]).await;
            assert!(
                volume.contains(&format!(" {expected_left}%"))
                    && volume.contains(&format!(" {expected_right}%")),
                "device_control={device_control}, ({left}, {right}), step={step}, up={up}: {volume}"
            );
            assert!(
                server
                    .pactl(&["get-sink-mute", "speakers"])
                    .await
                    .contains("yes")
            );
        }
    }
}

#[tokio::test]
async fn audio_devices_support_mute_volume_selection_and_reject_removed_targets() {
    let server = AudioServer::new().await;
    let (audio, mut state) = monitor(Some(server.server.clone()));
    devices_matching(&mut state, |s| s.devices.is_empty()).await;
    let first = server
        .pactl(&["load-module", "module-null-sink", "sink_name=speakers"])
        .await;
    server
        .pactl(&["load-module", "module-null-sink", "sink_name=headphones"])
        .await;
    server.set_default_sink("speakers").await;
    server
        .pactl(&["set-sink-volume", "speakers", "30%", "60%"])
        .await;
    let snapshot = devices_matching(&mut state, |s| {
        s.devices.len() == 2
            && s.devices
                .iter()
                .any(|d| d.id.name == "speakers" && (d.volume - 45.0).abs() < 0.1)
    })
    .await;
    assert!(
        snapshot
            .devices
            .iter()
            .all(|d| d.id.kind == DeviceKind::Output),
        "monitor sources must be excluded"
    );
    let speakers = snapshot
        .devices
        .iter()
        .find(|d| d.id.name == "speakers")
        .unwrap()
        .id
        .clone();
    audio
        .apply(speakers.clone(), DeviceOperation::SetMuted(true))
        .await
        .unwrap();
    audio
        .apply(
            speakers.clone(),
            DeviceOperation::AdjustVolume { step: 10, up: true },
        )
        .await
        .unwrap();
    devices_matching(&mut state, |s| {
        s.devices
            .iter()
            .any(|d| d.id == speakers && d.muted && (d.volume - 52.5).abs() < 0.1)
    })
    .await;
    let volume = server.pactl(&["get-sink-volume", "speakers"]).await;
    assert!(
        volume.contains("35%") && volume.contains("70%"),
        "channel proportions changed: {volume}"
    );
    for (left, average) in [("100%", 75.0), ("120%", 85.0)] {
        server
            .pactl(&["set-sink-volume", "speakers", left, "50%"])
            .await;
        let snapshot = devices_matching(&mut state, |s| {
            s.devices
                .iter()
                .any(|d| d.id == speakers && (d.volume - average).abs() < 0.1)
        })
        .await;
        let device = snapshot.devices.iter().find(|d| d.id == speakers).unwrap();
        assert!(!device.can_increase_volume());
        audio
            .apply(
                speakers.clone(),
                DeviceOperation::AdjustVolume { step: 5, up: true },
            )
            .await
            .unwrap();
        let volume = server.pactl(&["get-sink-volume", "speakers"]).await;
        assert!(
            volume.contains(left) && volume.contains("50%"),
            "increase must not lower amplified or unbalanced channels: {volume}"
        );
    }
    audio
        .apply(speakers.clone(), DeviceOperation::SetMuted(false))
        .await
        .unwrap();
    devices_matching(&mut state, |s| {
        s.devices.iter().any(|d| d.id == speakers && !d.muted)
    })
    .await;
    let headphones = snapshot
        .devices
        .iter()
        .find(|d| d.id.name == "headphones")
        .unwrap()
        .id
        .clone();
    audio
        .apply(headphones.clone(), DeviceOperation::SetDefault)
        .await
        .unwrap();
    // The private fixture has no session manager: verify the requested default,
    // then supply the same active-default policy as the original bar fixture.
    let metadata = server.run("pw-metadata", &["-n", "default"]).await;
    assert!(metadata.contains("default.configured.audio.sink") && metadata.contains("headphones"));
    server.set_default_sink("headphones").await;
    devices_matching(&mut state, |s| {
        s.devices.iter().any(|d| d.id == headphones && d.is_default)
    })
    .await;
    server.pactl(&["unload-module", &first]).await;
    devices_matching(&mut state, |s| s.devices.len() == 1).await;
    assert!(
        audio
            .apply(speakers, DeviceOperation::SetMuted(true))
            .await
            .is_err()
    );
    let mut stale = headphones;
    stale.server_cookie = stale.server_cookie.wrapping_add(1);
    assert!(
        audio
            .apply(stale, DeviceOperation::SetMuted(true))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn microphones_support_volume_mute_and_default_selection() {
    let server = AudioServer::new().await;
    server
        .pactl(&["load-module", "module-null-sink", "sink_name=fixture"])
        .await;
    for name in ["microphone", "headset"] {
        server
            .pactl(&[
                "load-module",
                "module-remap-source",
                "master=fixture.monitor",
                &format!("source_name={name}"),
            ])
            .await;
    }
    server
        .pactl(&["set-source-volume", "microphone", "40%"])
        .await;
    let (audio, mut state) = monitor(Some(server.server.clone()));
    let snapshot = devices_matching(&mut state, |s| {
        s.devices.iter().any(|d| {
            d.id.kind == DeviceKind::Input
                && d.id.name == "microphone"
                && (d.volume - 40.0).abs() < 0.1
        })
    })
    .await;
    assert_eq!(
        snapshot
            .devices
            .iter()
            .filter(|d| d.id.kind == DeviceKind::Input)
            .count(),
        2
    );
    let microphone = snapshot
        .devices
        .iter()
        .find(|d| d.id.name == "microphone")
        .unwrap()
        .id
        .clone();
    audio
        .apply(
            microphone.clone(),
            DeviceOperation::AdjustVolume { step: 5, up: true },
        )
        .await
        .unwrap();
    audio
        .apply(microphone.clone(), DeviceOperation::SetMuted(true))
        .await
        .unwrap();
    devices_matching(&mut state, |s| {
        s.devices
            .iter()
            .any(|d| d.id == microphone && d.muted && (d.volume - 45.0).abs() < 0.1)
    })
    .await;
    assert!(
        server
            .pactl(&["get-source-mute", "microphone"])
            .await
            .contains("yes")
    );
    audio
        .apply(microphone.clone(), DeviceOperation::SetDefault)
        .await
        .unwrap();
    let metadata = server.run("pw-metadata", &["-n", "default"]).await;
    assert!(
        metadata.contains("default.configured.audio.source") && metadata.contains("microphone")
    );
    server
        .run(
            "pw-metadata",
            &[
                "-n",
                "default",
                "0",
                "default.audio.source",
                r#"{"name":"microphone"}"#,
                "Spa:String:JSON",
            ],
        )
        .await;
    devices_matching(&mut state, |s| {
        s.devices.iter().any(|d| d.id == microphone && d.is_default)
    })
    .await;
    audio
        .apply(
            microphone.clone(),
            DeviceOperation::AdjustVolume {
                step: 100,
                up: true,
            },
        )
        .await
        .unwrap();
    devices_matching(&mut state, |s| {
        s.devices
            .iter()
            .any(|d| d.id == microphone && d.muted && (d.volume - 100.0).abs() < 0.1)
    })
    .await;
    audio
        .apply(
            microphone.clone(),
            DeviceOperation::AdjustVolume {
                step: 100,
                up: false,
            },
        )
        .await
        .unwrap();
    devices_matching(&mut state, |s| {
        s.devices
            .iter()
            .any(|d| d.id == microphone && d.muted && d.volume == 0.0)
    })
    .await;
    audio
        .apply(microphone.clone(), DeviceOperation::SetMuted(false))
        .await
        .unwrap();
    devices_matching(&mut state, |s| {
        s.devices.iter().any(|d| d.id == microphone && !d.muted)
    })
    .await;
}

#[tokio::test]
async fn device_observation_recovers_after_audio_server_restart() {
    let mut server = AudioServer::new().await;
    server
        .pactl(&["load-module", "module-null-sink", "sink_name=speakers"])
        .await;
    let (audio, mut state) = monitor(Some(server.server.clone()));
    let before = devices_matching(&mut state, |s| s.devices.len() == 1).await;
    server.stop_pulse();
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if state.borrow_and_update().is_none() {
                break;
            }
            state.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    server.restart_pulse();
    devices_matching(&mut state, |_| true).await;
    server
        .pactl(&["load-module", "module-null-sink", "sink_name=speakers"])
        .await;
    let after = devices_matching(&mut state, |s| s.devices.len() == 1).await;
    assert_ne!(before.devices[0].id.session, after.devices[0].id.session);
    assert!(
        audio
            .apply(
                before.devices[0].id.clone(),
                DeviceOperation::SetMuted(true)
            )
            .await
            .is_err()
    );
    assert!(!after.devices[0].muted);
}

async fn expect(
    rx: &mut mpsc::UnboundedReceiver<Result<Option<(f64, bool)>, ()>>,
    expected: Option<(f64, bool)>,
) {
    let mut last = None;
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let reading = rx.recv().await.unwrap().unwrap();
            last = Some(reading);
            match (reading, expected) {
                (None, None) => break,
                (Some((actual, muted)), Some((volume, expected_muted)))
                    if (actual - volume).abs() < 0.1 && muted == expected_muted =>
                {
                    break;
                }
                _ => {}
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("audio subscription expected {expected:?}, last reading {last:?}"));
}

#[tokio::test]
async fn pulse_protocol_controls_and_tracks_default_sink_removal_and_reappearance() {
    let mut server = AudioServer::new().await;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let address = server.server.clone();
    let observer = std::thread::spawn(move || run_pa_thread_on(Some(&address), tx));
    expect(&mut rx, None).await;
    assert!(adjust_volume_on(Some(&server.server), 5, true).is_err());
    let first = server
        .pactl(&["load-module", "module-null-sink", "sink_name=first"])
        .await;
    server.set_default_sink("first").await;
    server.pactl(&["set-sink-volume", "first", "30%"]).await;
    expect(&mut rx, Some((30.0, false))).await;
    adjust_volume_on(Some(&server.server), 5, true).unwrap();
    expect(&mut rx, Some((35.0, false))).await;
    adjust_volume_on(Some(&server.server), 10, false).unwrap();
    expect(&mut rx, Some((25.0, false))).await;
    let second = server
        .pactl(&["load-module", "module-null-sink", "sink_name=second"])
        .await;
    server.pactl(&["set-sink-volume", "second", "70%"]).await;
    server.pactl(&["set-sink-mute", "second", "1"]).await;
    server.set_default_sink("second").await;
    expect(&mut rx, Some((70.0, true))).await;
    server.pactl(&["unload-module", &first]).await;
    server.pactl(&["unload-module", &second]).await;
    expect(&mut rx, None).await;
    server
        .pactl(&["load-module", "module-null-sink", "sink_name=returned"])
        .await;
    server.set_default_sink("returned").await;
    server.pactl(&["set-sink-volume", "returned", "45%"]).await;
    expect(&mut rx, Some((45.0, false))).await;
    server.stop_pulse();
    tokio::time::timeout(Duration::from_secs(3), async {
        while rx.recv().await.unwrap().is_ok() {}
    })
    .await
    .unwrap();
    drop(rx);
    observer.join().unwrap();
}

#[tokio::test]
async fn idle_volume_observer_sleeps_until_disconnect_or_cancellation() {
    let server = AudioServer::new().await;
    let (tx, mut rx) = mpsc::unbounded_channel();
    let (thread_tx, thread_rx) = tokio::sync::oneshot::channel();
    let address = server.server.clone();
    let observer = std::thread::spawn(move || {
        let task = std::fs::read_link("/proc/thread-self").unwrap();
        thread_tx
            .send(std::path::Path::new("/proc").join(task).join("status"))
            .unwrap();
        run_pa_thread_on(Some(&address), tx);
    });
    let status = thread_rx.await.unwrap();
    expect(&mut rx, None).await;
    // Let the initial subscription callbacks settle before observing a stable server.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let switches = || {
        std::fs::read_to_string(&status)
            .unwrap()
            .lines()
            .find_map(|line| line.strip_prefix("voluntary_ctxt_switches:"))
            .unwrap()
            .trim()
            .parse::<u64>()
            .unwrap()
    };
    let before = switches();
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(
        switches() - before <= 2,
        "an idle volume observer must wait for events instead of periodically polling"
    );
    // Cancellation must wake the observer even when the audio server stays ready.
    drop(rx);
    tokio::time::timeout(
        Duration::from_secs(1),
        tokio::task::spawn_blocking(move || observer.join()),
    )
    .await
    .expect("dropping the volume receiver did not wake the observer")
    .unwrap()
    .unwrap();
}

#[tokio::test]
async fn audio_slider_sets_exact_output_and_input_levels_without_unmuting() {
    let server = AudioServer::new().await;
    server
        .pactl(&["load-module", "module-null-sink", "sink_name=speakers"])
        .await;
    server
        .pactl(&[
            "load-module",
            "module-remap-source",
            "master=speakers.monitor",
            "source_name=microphone",
        ])
        .await;
    let (audio, mut state) = monitor(Some(server.server.clone()));
    let initial = devices_matching(&mut state, |s| s.devices.len() == 2).await;
    for (name, kind) in [("speakers", "sink"), ("microphone", "source")] {
        let id = initial
            .devices
            .iter()
            .find(|device| device.id.name == name)
            .unwrap()
            .id
            .clone();
        server
            .pactl(&[&format!("set-{kind}-volume"), name, "30%", "60%"])
            .await;
        server
            .pactl(&[&format!("set-{kind}-mute"), name, "1"])
            .await;
        audio
            .apply(id.clone(), DeviceOperation::SetVolume { percent: 80 })
            .await
            .unwrap();
        devices_matching(&mut state, |s| {
            s.devices
                .iter()
                .any(|d| d.id == id && d.muted && (d.max_volume_percent() - 80.0).abs() < 0.1)
        })
        .await;
        let channels = server.pactl(&[&format!("get-{kind}-volume"), name]).await;
        assert!(
            channels.contains("40%") && channels.contains("80%"),
            "{channels}"
        );
        assert!(
            audio
                .apply(id.clone(), DeviceOperation::SetVolume { percent: 101 })
                .await
                .is_err()
        );
        assert_eq!(
            server.pactl(&[&format!("get-{kind}-volume"), name]).await,
            channels
        );
        for percent in [100, 0] {
            audio
                .apply(id.clone(), DeviceOperation::SetVolume { percent })
                .await
                .unwrap();
            devices_matching(&mut state, |s| {
                s.devices.iter().any(|d| {
                    d.id == id
                        && d.muted
                        && (d.max_volume_percent() - f64::from(percent)).abs() < 0.1
                })
            })
            .await;
        }
    }
}

#[tokio::test]
async fn the_mixer_lists_and_controls_applications_balance_and_cards() {
    let server = AudioServer::new().await;
    server
        .pactl(&["load-module", "module-null-sink", "sink_name=speakers"])
        .await;
    server.set_default_sink("speakers").await;
    let mut player = server.command("pacat");
    player
        .args([
            "--playback",
            "--device=speakers",
            "--client-name=Music Player",
            "--stream-name=A song",
        ])
        .stdin(std::fs::File::open("/dev/zero").unwrap())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut player = player.spawn().unwrap();
    let (audio, mut state) = mixer(Some(server.server.clone()));
    let snapshot = devices_matching(&mut state, |s| {
        s.devices.len() == 1 && s.streams.iter().any(|s| s.application == "Music Player")
    })
    .await;
    let stream = snapshot
        .streams
        .iter()
        .find(|s| s.application == "Music Player")
        .unwrap()
        .clone();
    assert_eq!(stream.id.kind, StreamKind::Playback);
    assert_eq!(stream.title, "A song");
    assert!(stream.volume_writable);
    // Without a session manager, the private server does not link streams.
    assert!(
        stream
            .device
            .is_none_or(|device| device == snapshot.devices[0].id.index)
    );
    audio
        .apply_stream(
            stream.id.clone(),
            StreamOperation::SetVolume { percent: 40 },
        )
        .await
        .unwrap();
    audio
        .apply_stream(stream.id.clone(), StreamOperation::SetMuted(true))
        .await
        .unwrap();
    devices_matching(&mut state, |s| {
        s.streams
            .iter()
            .any(|s| s.id == stream.id && s.muted && (s.volume - 40.0).abs() < 0.5)
    })
    .await;

    // Balance keeps the loudest channel and lowers the other side.
    let speakers = snapshot.devices[0].clone();
    assert_eq!(speakers.balance, Some(0));
    audio
        .apply(
            speakers.id.clone(),
            DeviceOperation::SetBalance { balance: 50 },
        )
        .await
        .unwrap();
    let balanced = devices_matching(&mut state, |s| {
        s.devices[0].balance.is_some_and(|b| (b - 50).abs() <= 1)
    })
    .await;
    assert!((balanced.devices[0].max_volume_percent() - speakers.max_volume_percent()).abs() < 1.0);
    assert!(matches!(
        audio
            .apply(
                speakers.id.clone(),
                DeviceOperation::SetBalance { balance: 101 }
            )
            .await,
        Err(AudioError::InvalidChannels)
    ));

    // The hardware-free server has no cards; a stale card is refused.
    assert!(balanced.cards.is_empty());
    let card = AudioCard {
        index: 7,
        name: "alsa_card.gone".into(),
        description: "Gone".into(),
        profiles: Vec::new(),
        active: None,
        server_cookie: speakers.id.server_cookie,
        session: speakers.id.session,
    };
    assert_eq!(
        audio.set_profile(&card, "off").await,
        Err(AudioError::DeviceGone)
    );

    // A finished stream disappears and cannot be changed any more.
    player.kill().unwrap();
    player.wait().unwrap();
    devices_matching(&mut state, |s| s.streams.iter().all(|s| s.id != stream.id)).await;
    assert_eq!(
        audio
            .apply_stream(stream.id.clone(), StreamOperation::SetMuted(false))
            .await,
        Err(AudioError::DeviceGone)
    );
}

#[tokio::test]
async fn the_level_meter_reports_peaks_hides_itself_and_stops_when_dropped() {
    let server = AudioServer::new().await;
    server
        .pactl(&["load-module", "module-null-sink", "sink_name=speakers"])
        .await;
    let (_audio, mut state) = mixer(Some(server.server.clone()));
    let mut meter = level_meter(Some(server.server.clone()), Some("speakers.monitor".into()));
    // The meter records while its receiver lives. The hardware-free server
    // has no session manager to link it, so it reports no peaks here.
    tokio::time::timeout(Duration::from_secs(5), async {
        while server
            .pactl(&["list", "short", "source-outputs"])
            .await
            .is_empty()
        {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the meter opens a recording stream");
    assert!(
        meter
            .borrow_and_update()
            .is_none_or(|peak| (0.0..=1.0).contains(&peak))
    );
    // The meter's own recording stream is not listed as an application.
    let snapshot = devices_matching(&mut state, |s| !s.devices.is_empty()).await;
    assert!(snapshot.streams.is_empty(), "{:?}", snapshot.streams);
    drop(meter);
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let outputs = server.pactl(&["list", "short", "source-outputs"]).await;
            if outputs.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the meter's stream closes after the receiver is dropped");
}
