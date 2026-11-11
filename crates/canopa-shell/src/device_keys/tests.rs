use super::*;
use canopa_services::testing::{
    TestBus,
    audio::AudioServer,
    logind::FakeLogind,
    mpris::{export, player},
};
use std::path::Path;
use std::sync::{Arc, Mutex};

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| word.to_string()).collect()
}

#[test]
fn commands_parse_with_optional_bounded_steps() {
    assert_eq!(
        parse(&args(&["volume", "up"])),
        Ok(Request {
            action: Action::VolumeUp,
            step: None
        })
    );
    assert_eq!(
        parse(&args(&["brightness", "down", "10"])).unwrap().step,
        Some(10)
    );
    assert_eq!(
        parse(&args(&["mic", "mute"])).unwrap().action,
        Action::MicrophoneMute
    );
    assert_eq!(
        parse(&args(&["media", "play-pause"])).unwrap().action,
        Action::MediaPlayPause
    );
    for refused in [
        &["volume"][..],
        &["volume", "sideways"],
        &["volume", "up", "0"],
        &["volume", "up", "101"],
        &["brightness", "up", "51"],
        &["volume", "mute", "5"],
        &["volume", "up", "5", "6"],
        &["media", "next", "1"],
    ] {
        assert!(parse(&args(refused)).is_err(), "{refused:?}");
    }
    assert!(is_command("keyboard-backlight") && is_command("mic") && !is_command("lock"));
    // Requests cross the IPC socket as JSON.
    let request = parse(&args(&["keyboard-backlight", "toggle"])).unwrap();
    let json = serde_json::to_string(&request).unwrap();
    assert_eq!(json, r#"{"action":"keyboard-backlight-toggle"}"#);
    assert_eq!(serde_json::from_str::<Request>(&json).unwrap(), request);
}

#[test]
fn indicators_describe_levels_mute_and_absence() {
    let _english = crate::i18n_testing::english();
    let speakers = Indicator::Level {
        channel: Channel::Volume,
        percent: 45,
        muted: false,
        device: Some("Speakers".into()),
    };
    assert_eq!(speakers.value(), "45%");
    assert_eq!(speakers.announcement(), "Volume 45%, Speakers");
    assert_eq!(speakers.fill(), Some(0.45));
    let amplified = Indicator::Level {
        channel: Channel::Volume,
        percent: 120,
        muted: false,
        device: None,
    };
    assert_eq!(amplified.fill(), Some(1.0));
    let off = Indicator::Level {
        channel: Channel::KeyboardBacklight,
        percent: 0,
        muted: true,
        device: None,
    };
    assert_eq!(off.announcement(), "Keyboard brightness off");
    let muted = Indicator::Level {
        channel: Channel::Microphone,
        percent: 80,
        muted: true,
        device: Some("Headset".into()),
    };
    assert_eq!(muted.announcement(), "Microphone muted, Headset");
    assert_eq!(
        Indicator::Unavailable(Channel::Brightness).announcement(),
        "Brightness unavailable"
    );
    assert_eq!(Outcome::NoPlayer.indicator(), None);
}

#[test]
fn indicators_are_announced_in_french() {
    let _french = crate::i18n_testing::select("fr", false);
    let speakers = Indicator::Level {
        channel: Channel::Volume,
        percent: 45,
        muted: false,
        device: Some("Haut-parleurs".into()),
    };
    assert_eq!(speakers.value(), "45\u{202f}%");
    assert_eq!(speakers.announcement(), "Volume 45\u{202f}%, Haut-parleurs");
    assert_eq!(
        Indicator::Unavailable(Channel::KeyboardBacklight).announcement(),
        "Luminosité du clavier indisponible"
    );
}

fn device(root: &Path, name: &str, kind: &str, max: u32, value: u32) {
    let path = root.join(name);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("type"), kind).unwrap();
    std::fs::write(path.join("max_brightness"), max.to_string()).unwrap();
    std::fs::write(path.join("brightness"), value.to_string()).unwrap();
}

/// A sysfs stand-in whose devices follow the fake logind's writes.
struct Lights {
    _root: tempfile::TempDir,
    backlights: PathBuf,
    leds: PathBuf,
    logind: FakeLogind,
}

impl Lights {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let (backlights, leds) = (root.path().join("backlight"), root.path().join("leds"));
        std::fs::create_dir_all(&backlights).unwrap();
        std::fs::create_dir_all(&leds).unwrap();
        Self {
            _root: root,
            backlights,
            leds,
            logind: FakeLogind::default(),
        }
    }

    /// Apply the writes logind received, as the kernel would.
    fn sync(&self) {
        for (subsystem, name, level) in self.logind.brightness.lock().unwrap().drain(..) {
            let root = if subsystem == "backlight" {
                &self.backlights
            } else {
                &self.leds
            };
            std::fs::write(root.join(name).join("brightness"), level.to_string()).unwrap();
        }
    }
}

async fn press(keys: &Keys, lights: &Lights, action: Action) -> Outcome {
    let outcome = keys
        .run(Request { action, step: None }, Steps::default())
        .await;
    lights.sync();
    outcome
}

#[tokio::test]
async fn brightness_steps_perceptually_through_logind_and_never_goes_dark() {
    let lights = Lights::new();
    let bus = TestBus::new();
    let _logind = lights.logind.serve(&bus).await;
    let keys = Keys::new(None, lights.backlights.clone(), lights.leds.clone())
        .with_buses(Some(bus.connect().await), None);
    assert_eq!(
        press(&keys, &lights, Action::BrightnessUp).await,
        Outcome::Unavailable(Channel::Brightness),
        "no backlight"
    );
    device(&lights.backlights, "intel_backlight", "raw", 1000, 1000);
    let percent = |outcome: Outcome| match outcome {
        Outcome::Changed(Indicator::Level { percent, .. }) => percent,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        percent(press(&keys, &lights, Action::BrightnessDown).await),
        95
    );
    assert_eq!(
        percent(press(&keys, &lights, Action::BrightnessDown).await),
        90
    );
    for _ in 0..40 {
        press(&keys, &lights, Action::BrightnessDown).await;
    }
    let floor = Backlight::discover(&lights.backlights).unwrap();
    assert_eq!(floor.value, floor.minimum(), "lowest press stays lit");
    assert_eq!(
        percent(press(&keys, &lights, Action::BrightnessDown).await),
        10
    );
    // A custom step applies.
    let outcome = keys
        .run(
            Request {
                action: Action::BrightnessUp,
                step: Some(20),
            },
            Steps::default(),
        )
        .await;
    lights.sync();
    assert_eq!(percent(outcome), 30);
    for _ in 0..20 {
        press(&keys, &lights, Action::BrightnessUp).await;
    }
    assert_eq!(Backlight::discover(&lights.backlights).unwrap().value, 1000);
    assert_eq!(
        percent(press(&keys, &lights, Action::BrightnessUp).await),
        100
    );
}

#[tokio::test]
async fn brightness_without_logind_is_unavailable() {
    let lights = Lights::new();
    device(&lights.backlights, "acpi_video0", "firmware", 10, 5);
    let bus = TestBus::new();
    let keys = Keys::new(None, lights.backlights.clone(), lights.leds.clone())
        .with_buses(Some(bus.connect().await), None);
    assert_eq!(
        keys.run(
            Request {
                action: Action::BrightnessUp,
                step: None
            },
            Steps::default()
        )
        .await,
        Outcome::Unavailable(Channel::Brightness)
    );
}

#[tokio::test]
async fn keyboard_backlight_steps_and_toggles_back_to_its_last_level() {
    let lights = Lights::new();
    let bus = TestBus::new();
    let _logind = lights.logind.serve(&bus).await;
    let keys = Keys::new(None, lights.backlights.clone(), lights.leds.clone())
        .with_buses(Some(bus.connect().await), None);
    assert_eq!(
        press(&keys, &lights, Action::KeyboardBacklightToggle).await,
        Outcome::Unavailable(Channel::KeyboardBacklight)
    );
    device(&lights.leds, "tpacpi::kbd_backlight", "", 2, 0);
    let shown = |outcome: Outcome| match outcome {
        Outcome::Changed(indicator) => (indicator.value(), indicator.muted()),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        shown(press(&keys, &lights, Action::KeyboardBacklightUp).await),
        ("50%".into(), false)
    );
    assert_eq!(
        shown(press(&keys, &lights, Action::KeyboardBacklightToggle).await),
        ("Off".into(), true)
    );
    assert_eq!(
        shown(press(&keys, &lights, Action::KeyboardBacklightToggle).await),
        ("50%".into(), false),
        "toggling restores the level it turned off"
    );
    assert_eq!(
        shown(press(&keys, &lights, Action::KeyboardBacklightUp).await),
        ("100%".into(), false)
    );
    assert_eq!(
        shown(press(&keys, &lights, Action::KeyboardBacklightUp).await),
        ("100%".into(), false),
        "clamped at the maximum"
    );
    for _ in 0..3 {
        press(&keys, &lights, Action::KeyboardBacklightDown).await;
    }
    assert_eq!(KeyboardBacklight::discover_all(&lights.leds)[0].value, 0);
    // Off by other means: toggling turns on at the maximum.
    let fresh = Keys::new(None, lights.backlights.clone(), lights.leds.clone())
        .with_buses(Some(bus.connect().await), None);
    assert_eq!(
        shown(press(&fresh, &lights, Action::KeyboardBacklightToggle).await),
        ("100%".into(), false)
    );
}

#[tokio::test]
async fn volume_keys_step_clamp_unmute_and_report_the_device() {
    let server = AudioServer::new().await;
    let keys = Keys::new(
        Some(server.server.clone()),
        PathBuf::from("/nonexistent"),
        PathBuf::from("/nonexistent"),
    );
    let run = |action, step| keys.run(Request { action, step }, Steps::default());
    assert_eq!(
        run(Action::VolumeUp, None).await,
        Outcome::Unavailable(Channel::Volume),
        "no output device"
    );
    assert_eq!(
        run(Action::MicrophoneMute, None).await,
        Outcome::Unavailable(Channel::Microphone)
    );
    server
        .pactl(&[
            "load-module",
            "module-null-sink",
            "sink_name=speakers",
            "sink_properties=device.description=Speakers",
        ])
        .await;
    server.set_default_sink("speakers").await;
    server.pactl(&["set-sink-volume", "speakers", "93%"]).await;
    server.pactl(&["set-sink-mute", "speakers", "1"]).await;
    let Outcome::Changed(raised) = run(Action::VolumeUp, None).await else {
        panic!("raise");
    };
    assert_eq!(raised.value(), "98%");
    assert_eq!(raised.announcement(), "Volume 98%, Speakers");
    assert_eq!(
        run(Action::VolumeUp, None)
            .await
            .indicator()
            .unwrap()
            .value(),
        "100%"
    );
    assert_eq!(
        run(Action::VolumeDown, Some(30))
            .await
            .indicator()
            .unwrap()
            .value(),
        "70%"
    );
    assert!(
        run(Action::VolumeMute, None)
            .await
            .indicator()
            .unwrap()
            .muted()
    );
    // The configured step applies when the key gives none.
    let lowered = keys
        .run(
            Request {
                action: Action::VolumeDown,
                step: None,
            },
            Steps {
                volume: 20,
                ..Steps::default()
            },
        )
        .await;
    assert_eq!(
        lowered.indicator(),
        Some(Indicator::Level {
            channel: Channel::Volume,
            percent: 50,
            muted: true,
            device: Some("Speakers".into())
        })
    );
}

/// A stand-in for the Shell's media source that records `Apply` requests.
#[derive(Clone)]
struct ShellMedia {
    snapshot: crate::media::Snapshot,
    applied: Arc<Mutex<Vec<String>>>,
}

#[zbus::interface(name = "org.canopa.Shell.Media")]
impl ShellMedia {
    fn snapshot(&self) -> String {
        serde_json::to_string(&self.snapshot).unwrap()
    }
    fn apply(&self, request: &str) {
        self.applied.lock().unwrap().push(request.into());
    }
}

#[tokio::test]
async fn media_keys_follow_the_shell_selection_or_pick_the_playing_player() {
    let bus = TestBus::new();
    let session = bus.connect().await;
    let keys = Keys::default().with_buses(None, Some(session.clone()));
    let run = |action| keys.run(Request { action, step: None }, Steps::default());
    assert_eq!(run(Action::MediaPlayPause).await, Outcome::NoPlayer);

    let (_paused, paused) = export(&bus, player("paused", "Paused")).await;
    let (_playing, playing) = export(&bus, player("playing", "Playing")).await;
    // No Shell media source: the playing player, as automatic selection.
    assert_eq!(run(Action::MediaPlayPause).await, Outcome::Played);
    assert_eq!(*playing.1.lock().unwrap(), ["Pause"]);
    assert_eq!(run(Action::MediaStop).await, Outcome::Played);
    assert_eq!(
        run(Action::MediaPrevious).await,
        Outcome::NoPlayer,
        "unsupported"
    );
    assert!(paused.1.lock().unwrap().is_empty());

    // With a Shell selection, keys go through it, even to a paused player.
    let mut snapshot = crate::media::Snapshot::default();
    snapshot.reconcile(canopa_services::mpris::players(&session).await.unwrap());
    let chosen = snapshot
        .players
        .iter()
        .find(|p| p.name == "paused")
        .unwrap()
        .id
        .clone();
    snapshot.selected = Some(chosen.clone());
    snapshot.pinned = true;
    let shell = ShellMedia {
        snapshot,
        applied: Arc::default(),
    };
    let _server = zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .name(crate::media::SERVICE)
        .unwrap()
        .serve_at(crate::media::PATH, shell.clone())
        .unwrap()
        .build()
        .await
        .unwrap();
    assert_eq!(run(Action::MediaPlayPause).await, Outcome::Played);
    let applied = shell.applied.lock().unwrap().clone();
    assert_eq!(applied.len(), 1);
    let crate::media::Request::Control {
        player, operation, ..
    } = serde_json::from_str(&applied[0]).unwrap()
    else {
        panic!("control request");
    };
    assert_eq!((player, operation), (chosen, Operation::Play));
    assert_eq!(*playing.1.lock().unwrap(), ["Pause", "Stop"]);
}
