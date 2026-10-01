//! Real Pulse protocol tests with private PipeWire daemons and synthetic sinks.
//! These configs load no hardware discovery or desktop session manager.
use super::*;
use std::process::{Child, Command, Stdio};

struct AudioServer {
    children: Vec<Child>,
    directory: tempfile::TempDir,
    server: String,
}

impl AudioServer {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let mut server = Self {
            server: format!("unix:{}/pulse/native", directory.path().display()),
            directory,
            children: Vec::new(),
        };
        std::fs::write(
            server.directory.path().join("client.conf"),
            r#"
            context.modules = [
                { name = libpipewire-module-protocol-native }
                { name = libpipewire-module-metadata }
            ]
        "#,
        )
        .unwrap();
        let core_config = r#"
            context.properties = { core.daemon = true core.name = pipewire-0 }
            context.spa-libs = { audio.convert.* = audioconvert/libspa-audioconvert support.* = support/libspa-support }
            context.modules = [
                { name = libpipewire-module-protocol-native }
                { name = libpipewire-module-access }
                { name = libpipewire-module-metadata }
                { name = libpipewire-module-spa-node-factory }
                { name = libpipewire-module-client-node }
                { name = libpipewire-module-adapter }
                { name = libpipewire-module-link-factory }
            ]
            context.objects = [ { factory = metadata args = { metadata.name = default } } ]
        "#;
        let pulse_config = r#"
            context.properties = { }
            context.spa-libs = { audio.convert.* = audioconvert/libspa-audioconvert support.* = support/libspa-support }
            context.modules = [
                { name = libpipewire-module-protocol-native }
                { name = libpipewire-module-client-node }
                { name = libpipewire-module-adapter }
                { name = libpipewire-module-metadata }
                { name = libpipewire-module-protocol-pulse args = { server.address = [ "unix:native" ] } }
            ]
        "#;
        for (program, config) in [("pipewire", core_config), ("pipewire-pulse", pulse_config)] {
            let path = server.directory.path().join(format!("{program}.conf"));
            std::fs::write(&path, config).unwrap();
            let log = std::fs::File::create(server.directory.path().join(format!("{program}.log")))
                .unwrap();
            let child = server
                .command(program)
                .arg("-c")
                .arg(path.file_name().unwrap())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(log)
                .spawn()
                .expect("install pipewire and pipewire-pulse for the audio transport tests");
            server.children.push(child);
            let socket = if program == "pipewire" {
                "pipewire-0"
            } else {
                "pulse/native"
            };
            let deadline = Instant::now() + Duration::from_secs(3);
            while !server.directory.path().join(socket).exists() {
                assert!(
                    Instant::now() < deadline,
                    "{program} failed: {}",
                    std::fs::read_to_string(server.directory.path().join(format!("{program}.log")))
                        .unwrap()
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        server.pactl(&["info"]).await;
        server
    }

    fn command(&self, program: &str) -> Command {
        let mut command = Command::new(program);
        command
            .env("XDG_RUNTIME_DIR", self.directory.path())
            .env("XDG_CONFIG_HOME", self.directory.path())
            .env("PIPEWIRE_RUNTIME_DIR", self.directory.path())
            .env("PIPEWIRE_CONFIG_DIR", self.directory.path())
            .env("PIPEWIRE_CONFIG_PREFIX", "")
            .env("PIPEWIRE_REMOTE", "pipewire-0")
            .env("PULSE_RUNTIME_PATH", self.directory.path().join("pulse"))
            .env("PULSE_SERVER", &self.server)
            .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent");
        command
    }

    async fn pactl(&self, args: &[&str]) -> String {
        self.run("pactl", args).await
    }

    async fn set_default_sink(&self, name: &str) {
        // A real session manager copies the configured choice into active
        // default metadata. This hardware-free fixture supplies that policy.
        self.pactl(&["set-default-sink", name]).await;
        self.run(
            "pw-metadata",
            &[
                "-n",
                "default",
                "0",
                "default.audio.sink",
                &format!(r#"{{"name":"{name}"}}"#),
                "Spa:String:JSON",
            ],
        )
        .await;
        self.pactl(&["get-sink-volume", "@DEFAULT_SINK@"]).await;
    }

    async fn run(&self, program: &str, args: &[&str]) -> String {
        let mut command = tokio::process::Command::from(self.command(program));
        command.args(args).kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(3), command.output())
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "private {program} {args:?} deadline; core: {}; pulse: {}",
                    std::fs::read_to_string(self.directory.path().join("pipewire.log")).unwrap(),
                    std::fs::read_to_string(self.directory.path().join("pipewire-pulse.log"))
                        .unwrap()
                )
            })
            .expect("install pipewire-bin and pulseaudio-utils for audio tests");
        assert!(
            output.status.success(),
            "{program} {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().into()
    }
}

impl Drop for AudioServer {
    fn drop(&mut self) {
        for child in self.children.iter_mut().rev() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

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
    server.children.last_mut().unwrap().kill().unwrap();
    server.children.last_mut().unwrap().wait().unwrap();
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
    let child = server
        .command("pipewire-pulse")
        .args(["-c", "pipewire-pulse.conf"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    server.children.push(child);
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

#[tokio::test]
async fn audio_panel_v5_controls_devices_and_ignores_old_revisions() {
    use canopa_panel_protocol::{InteractionEvent, InteractionTarget, v5};
    use tokio::io::{AsyncWriteExt, BufReader};
    let server = AudioServer::new().await;
    server
        .pactl(&["load-module", "module-null-sink", "sink_name=speakers"])
        .await;
    server
        .pactl(&["set-sink-volume", "speakers", "100%", "50%"])
        .await;
    let (host, provider) = tokio::io::duplex(65536);
    let (input, output) = tokio::io::split(provider);
    let address = server.server.clone();
    let running = tokio::spawn(crate::audio_provider::run(
        BufReader::new(input),
        output,
        Some(address),
    ));
    let (input, mut output) = tokio::io::split(host);
    let mut input = BufReader::new(input);
    async fn next(
        reader: &mut (impl tokio::io::AsyncBufRead + Unpin),
        muted: bool,
        volume: Option<u16>,
    ) -> v5::Response {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let response = v5::Response::read_async(reader).await.unwrap().unwrap();
                let v5::Content::Rich(content) = &response.content else {
                    panic!("rich content")
                };
                if content
                    .sections
                    .iter()
                    .flat_map(|section| &section.items)
                    .any(|item| {
                        volume.is_none_or(|value| {
                            item.slider
                                .as_ref()
                                .is_some_and(|slider| slider.value == value)
                        }) && item.actions.iter().any(|action| {
                            action.key == "mute"
                                && action.label == if muted { "Unmute" } else { "Mute" }
                        }) && !item
                            .body
                            .as_deref()
                            .unwrap_or_default()
                            .contains("Applying")
                    })
                {
                    return response;
                }
            }
        })
        .await
        .unwrap()
    }
    let initial = next(&mut input, false, None).await;
    let v5::Content::Rich(content) = &initial.content else {
        unreachable!()
    };
    let item = &content.sections[0].items[0];
    assert!(
        !item.actions.iter().any(|action| action.key == "louder"),
        "a capped channel must hide volume increase even when the average is lower"
    );
    assert_eq!(
        item.slider.as_ref().unwrap().value,
        100,
        "slider shows the loudest channel, not its average"
    );
    let item_id = item.id.clone();
    let invoke = v5::HostMessage::event(
        initial.revision,
        v5::Interaction::Rich(InteractionEvent::Invoke {
            target: InteractionTarget::Item {
                id: item.id.clone(),
            },
            action_key: "mute".into(),
            value: None,
        }),
    );
    output.write_all(&invoke.encode().unwrap()).await.unwrap();
    let updated = next(&mut input, true, None).await;
    assert!(updated.revision > initial.revision);
    assert!(
        server
            .pactl(&["get-sink-mute", "speakers"])
            .await
            .contains("yes")
    );
    // Return the hardware to unmuted externally, then submit an old "mute"
    // frame. Accepting it would wrongly mute the device again.
    server.pactl(&["set-sink-mute", "speakers", "0"]).await;
    let current = next(&mut input, false, None).await;
    output.write_all(&invoke.encode().unwrap()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        server
            .pactl(&["get-sink-mute", "speakers"])
            .await
            .contains("no")
    );
    let set_volume = v5::HostMessage::event(
        current.revision,
        v5::Interaction::Rich(InteractionEvent::Invoke {
            target: InteractionTarget::Item {
                id: item_id.clone(),
            },
            action_key: "volume".into(),
            value: Some("64".into()),
        }),
    );
    output
        .write_all(&set_volume.encode().unwrap())
        .await
        .unwrap();
    let current = next(&mut input, false, Some(64)).await;
    let channels = server.pactl(&["get-sink-volume", "speakers"]).await;
    assert!(
        channels.contains("64%") && channels.contains("32%"),
        "{channels}"
    );
    // Stall only this private Pulse server so multiple valid releases arrive
    // while the first command is still pending. Accepted edits must survive the
    // intermediate observer publication, and the following mute must also run.
    let pulse_pid = server.children.last().unwrap().id().to_string();
    server.run("kill", &["-STOP", &pulse_pid]).await;
    let mut batch = Vec::new();
    for (action_key, value) in [
        ("volume", Some("42")),
        ("volume", Some("57")),
        ("volume", Some("64")),
        ("mute", None),
    ] {
        batch.extend(
            v5::HostMessage::event(
                current.revision,
                v5::Interaction::Rich(InteractionEvent::Invoke {
                    target: InteractionTarget::Item {
                        id: item_id.clone(),
                    },
                    action_key: action_key.into(),
                    value: value.map(str::to_owned),
                }),
            )
            .encode()
            .unwrap(),
        );
    }
    output.write_all(&batch).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    server.run("kill", &["-CONT", &pulse_pid]).await;
    next(&mut input, true, Some(64)).await;
    let channels = server.pactl(&["get-sink-volume", "speakers"]).await;
    assert!(
        channels.contains("64%") && channels.contains("32%"),
        "{channels}"
    );
    assert!(
        server
            .pactl(&["get-sink-mute", "speakers"])
            .await
            .contains("yes")
    );
    output
        .write_all(&v5::HostMessage::close().encode().unwrap())
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(1), running)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
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
    server.children.last_mut().unwrap().kill().unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while rx.recv().await.unwrap().is_ok() {}
    })
    .await
    .unwrap();
    drop(rx);
    observer.join().unwrap();
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
