use canopa_settings::{
    display::{Monitor, TrialRequest},
    input::Input,
    niri::Client,
    preferences::{IdleTimes, Personal},
    profiles::{Book, Switching},
    session::Paths,
    storage::Snapshot,
};
use niri_ipc::{LogicalOutput, Mode, Output, OutputAction, Request, Response, Transform};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    os::unix::{fs::symlink, net::UnixListener},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[test]
fn idle_acknowledgements_reject_native_exit_zero_errors() {
    use canopa_settings::preferences::validate_idle_reply;
    assert!(validate_idle_reply("paused").is_ok());
    assert!(validate_idle_reply("Reloaded successfully").is_ok());
    for reply in ["", "ERROR: Invalid configuration", "Stasis is not running"] {
        assert!(validate_idle_reply(reply).is_err(), "{reply}");
    }
}

#[test]
fn subprocess_output_is_bounded_and_failure_is_reported() {
    use canopa_settings::storage::command;
    assert!(
        command("python3", &["-c".into(), "print('x' * 1048577)".into()])
            .unwrap_err()
            .to_string()
            .contains("budget")
    );
    assert!(
        command(
            "python3",
            &[
                "-c".into(),
                "import sys;sys.stderr.write('owner rejected');sys.exit(1)".into()
            ]
        )
        .unwrap_err()
        .to_string()
        .contains("owner rejected")
    );
    assert!(
        command(
            "python3",
            &[
                "-c".into(),
                "print('invalid clock format');raise SystemExit(1)".into()
            ]
        )
        .unwrap_err()
        .to_string()
        .contains("invalid clock format")
    );
}
#[test]
fn launched_application_errors_are_reported_and_success_is_reaped() {
    use canopa_settings::storage::application;
    application(
        "python3",
        &["-c".into(), "print('application closed')".into()],
    )
    .unwrap();
    assert!(
        application(
            "python3",
            &[
                "-c".into(),
                "import sys;sys.stderr.write('no wallpaper images');sys.exit(1)".into()
            ]
        )
        .unwrap_err()
        .to_string()
        .contains("no wallpaper images")
    );
}

fn paths(root: &std::path::Path) -> Paths {
    let paths = Paths {
        directory: root.join("config/canopa"),
    };
    paths
        .initialize(Some(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../packaging/niri/defaults.kdl")
                .canonicalize()
                .unwrap(),
        ))
        .unwrap();
    paths
}
#[test]
#[ignore = "requires a native Niri parser; the desktop workflow runs this check"]
fn separate_session_defaults_validate_and_existing_files_are_preserved() {
    let root = tempfile::tempdir().unwrap();
    let paths = paths(root.path());
    canopa_settings::niri::validate(&paths.niri()).unwrap();
    let connected = output();
    let mut monitor = Monitor::from_output(&connected);
    monitor.scale = 1.5;
    monitor.mode = Some(1);
    monitor.x = 120;
    let outputs = HashMap::from([(connected.name.clone(), connected)]);
    let source = Snapshot::load(paths.displays()).unwrap();
    let text = canopa_settings::display::text(&source, &[monitor], &outputs).unwrap();
    source
        .save(&text, |candidate| {
            canopa_settings::niri::validate_fragment(&paths, "niri-displays.kdl", candidate)
        })
        .unwrap();
    canopa_settings::niri::validate(&paths.niri()).unwrap();
    std::fs::write(paths.input(), "// user note\n").unwrap();
    paths.initialize(None).unwrap();
    assert_eq!(
        std::fs::read_to_string(paths.input()).unwrap(),
        "// user note\n"
    );
    assert!(!root.path().join("config/niri/config.kdl").exists());
}
#[test]
#[ignore = "requires a native Niri parser; the desktop workflow runs this check"]
fn input_round_trips_and_actual_niri_validates_the_generated_include() {
    let root = tempfile::tempdir().unwrap();
    let paths = paths(root.path());
    let input = Input {
        layouts: "us,fr".into(),
        options: "grp:alt_shift_toggle".into(),
        mouse_speed: -0.4,
        ..Default::default()
    };
    let source = Snapshot::load(paths.input()).unwrap();
    let text = input.text().unwrap();
    assert_eq!(Input::parse(&text).unwrap(), input);
    source
        .save(&text, |candidate| {
            canopa_settings::niri::validate_fragment(&paths, "niri-input.kdl", candidate)
        })
        .unwrap();
    let current = Snapshot::load(paths.input()).unwrap();
    assert!(
        current
            .save("input { not-a-niri-setting; }\n", |candidate| {
                canopa_settings::niri::validate_fragment(&paths, "niri-input.kdl", candidate)
            })
            .is_err()
    );
    assert_eq!(std::fs::read_to_string(paths.input()).unwrap(), text);
    let invalid = Input {
        layouts: "not_a_real_layout".into(),
        ..Default::default()
    };
    assert!(invalid.validate().is_err());
}
#[test]
fn stale_sources_validation_errors_and_symlinks_cannot_destroy_configuration() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    std::fs::write(&path, "schema-version 1\n").unwrap();
    let snapshot = Snapshot::load(&path).unwrap();
    assert!(
        snapshot
            .save("new", |_| anyhow::bail!("rejected by owner"))
            .is_err()
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), snapshot.text);
    std::fs::write(&path, "// external\nschema-version 1\n").unwrap();
    assert!(
        snapshot
            .save("new", |_| Ok(()))
            .unwrap_err()
            .to_string()
            .contains("outside this editor")
    );
    std::fs::remove_file(&path).unwrap();
    let original = root.path().join("original");
    std::fs::write(&original, "keep").unwrap();
    symlink(&original, &path).unwrap();
    assert!(Snapshot::load(&path).is_err());
    assert_eq!(std::fs::read_to_string(original).unwrap(), "keep");
}
#[test]
fn personalization_preserves_other_namespaces_comments_and_custom_lock_fields() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    std::fs::write(&path,"schema-version 1\n// retained note\nnotifications { sound enabled=#false; }\nlockscreen { auth-timeout 42;\n// widget note\nwidgets \"keyboard\"; }\n").unwrap();
    let snapshot = Snapshot::load(&path).unwrap();
    let mut value = Personal::load(&snapshot).unwrap();
    value.appearance.color_scheme = canopa_theme::ColorScheme::Light;
    value.lock.dim = 72;
    let text = value.text(&snapshot).unwrap();
    assert!(text.contains("retained note"));
    assert!(text.contains("widget note"));
    assert!(text.contains("auth-timeout 42"));
    let document = canopa_config::Document::parse(
        text.clone(),
        Some(path),
        canopa_config::Format::CanopaV1,
        canopa_config::Component::Lockscreen,
        "",
    )
    .unwrap();
    let parsed = canopa_session_ui::settings::Settings::parse(
        document.selected(),
        document.appearance(),
        None,
        canopa_config::Component::Lockscreen,
    )
    .unwrap();
    assert_eq!(parsed.dim, 72);
    assert_eq!(parsed.auth_timeout, 42);
    assert!(text.contains("enabled=#false"));
}
#[test]
fn repeated_personalization_edits_remain_valid_for_the_owning_schema() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config.kdl");
    let source = Snapshot::load(&path).unwrap();
    let mut preferences = Personal::load(&source).unwrap();
    preferences.appearance.color_scheme = canopa_theme::ColorScheme::Light;
    std::fs::write(&path, preferences.text(&source).unwrap()).unwrap();
    let source = Snapshot::load(&path).unwrap();
    let mut preferences = Personal::load(&source).unwrap();
    preferences.lock.clock_format = "%H:%M:%S".into();
    std::fs::write(&path, preferences.text(&source).unwrap()).unwrap();
    let reloaded = Personal::load(&Snapshot::load(&path).unwrap()).unwrap();
    assert_eq!(reloaded.lock.clock_format, "%H:%M:%S");
    assert_eq!(
        reloaded.appearance.color_scheme,
        canopa_theme::ColorScheme::Light
    );
}
#[test]
fn idle_edits_only_numeric_tokens_and_preserves_commands_and_profiles() {
    let original = include_str!("../../../packaging/stasis/stasis.rune");
    let mut times = IdleTimes::parse(original).unwrap();
    times.ac[0] = 900;
    times.battery[2] = 120;
    let text = times.text(original).unwrap();
    assert_eq!(IdleTimes::parse(&text).unwrap(), times);
    assert_eq!(
        original
            .lines()
            .filter(|l| !l.trim_start().starts_with("timeout"))
            .collect::<Vec<_>>(),
        text.lines()
            .filter(|l| !l.trim_start().starts_with("timeout"))
            .collect::<Vec<_>>()
    );
    assert!(
        times
            .text("default:\n lock_screen:\n timeout 30\n end\nend\n")
            .is_err()
    );
}

fn output() -> Output {
    Output {
        name: "TEST-1".into(),
        make: "Canopa".into(),
        model: "Fixture".into(),
        serial: Some("one".into()),
        physical_size: Some((300, 200)),
        modes: vec![
            Mode {
                width: 1280,
                height: 800,
                refresh_rate: 60000,
                is_preferred: true,
            },
            Mode {
                width: 1024,
                height: 768,
                refresh_rate: 60000,
                is_preferred: false,
            },
        ],
        current_mode: Some(0),
        is_custom_mode: false,
        vrr_supported: false,
        vrr_enabled: false,
        logical: Some(LogicalOutput {
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
            scale: 1.,
            transform: Transform::Normal,
        }),
    }
}
#[test]
fn display_saves_preserve_unmanaged_output_options_and_disconnected_outputs() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("displays.kdl");
    std::fs::write(&path, "output \"TEST-1\" {\n // user option\n variable-refresh-rate\n scale 2\n}\noutput \"other\" { off; }\n").unwrap();
    let source = Snapshot::load(&path).unwrap();
    let output = output();
    let monitors = [Monitor::from_output(&output)];
    let outputs = HashMap::from([(output.name.clone(), output)]);
    let text = canopa_settings::display::text(&source, &monitors, &outputs).unwrap();
    assert!(text.contains("user option"));
    assert!(text.contains("variable-refresh-rate"));
    assert!(text.contains("\"other\""));
    let parsed = kdl::KdlDocument::parse_v1(&text).unwrap();
    let managed = parsed
        .nodes()
        .iter()
        .find(|node| node.get(0).and_then(|v| v.as_string()) == Some("TEST-1"))
        .unwrap();
    assert_eq!(
        managed
            .children()
            .unwrap()
            .get("scale")
            .unwrap()
            .get(0)
            .unwrap()
            .as_integer(),
        Some(1)
    );
}
struct Peer {
    path: std::path::PathBuf,
    output: Arc<Mutex<Output>>,
    fail_scale: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}
impl Peer {
    fn start(root: &std::path::Path) -> Self {
        let path = root.join("niri.sock");
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let output = Arc::new(Mutex::new(output()));
        let current = output.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let fail_scale = Arc::new(AtomicBool::new(false));
        let fail = fail_scale.clone();
        let join = std::thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut line = String::new();
                if BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut line)
                    .is_err()
                {
                    continue;
                }
                let request: Request = serde_json::from_str(&line).unwrap();
                let reply: niri_ipc::Reply = match request {
                    Request::Outputs => Ok(Response::Outputs(HashMap::from([(
                        "TEST-1".into(),
                        current.lock().unwrap().clone(),
                    )]))),
                    Request::Output {
                        output: name,
                        action,
                    } => {
                        assert_eq!(name, "TEST-1");
                        let mut output = current.lock().unwrap();
                        let failed = matches!(action,OutputAction::Scale{scale:niri_ipc::ScaleToSet::Specific(value)} if value==1.5)
                            && fail.swap(false, Ordering::Relaxed);
                        if failed {
                            Err("fixture scale failure".into())
                        } else {
                            match action {
                                OutputAction::Off => {
                                    output.current_mode = None;
                                    output.logical = None;
                                }
                                OutputAction::On => {
                                    output.current_mode.get_or_insert(0);
                                    output.logical.get_or_insert(output::logical());
                                }
                                OutputAction::Mode {
                                    mode: niri_ipc::ModeToSet::Specific(mode),
                                }
                                | OutputAction::CustomMode { mode } => {
                                    output.current_mode = output.modes.iter().position(|m| {
                                        m.width == mode.width && m.height == mode.height
                                    });
                                }
                                OutputAction::Scale {
                                    scale: niri_ipc::ScaleToSet::Specific(value),
                                } => output.logical.as_mut().unwrap().scale = value,
                                OutputAction::Transform { transform } => {
                                    output.logical.as_mut().unwrap().transform = transform
                                }
                                OutputAction::Position {
                                    position: niri_ipc::PositionToSet::Specific(position),
                                } => {
                                    let logical = output.logical.as_mut().unwrap();
                                    logical.x = position.x;
                                    logical.y = position.y;
                                }
                                _ => {}
                            }
                            Ok(Response::OutputConfigChanged(
                                niri_ipc::OutputConfigChanged::Applied,
                            ))
                        }
                    }
                    Request::Action(_) => Ok(Response::Handled),
                    _ => Err("unexpected fixture request".into()),
                };
                serde_json::to_writer(&mut stream, &reply).unwrap();
                let _ = stream.write_all(b"\n");
            }
        });
        Self {
            path,
            output,
            fail_scale,
            stop,
            join: Some(join),
        }
    }
    fn start_guard(&self, paths: &Paths) -> std::process::Child {
        self.start_guard_with(paths, &[])
    }
    fn start_guard_with(&self, paths: &Paths, args: &[&str]) -> std::process::Child {
        Command::new(assert_cmd::cargo::cargo_bin!("canopa-settings"))
            .arg("display-guard")
            .args(args)
            .env("NIRI_SOCKET", &self.path)
            .env("NIRI_CONFIG", paths.niri())
            .env("XDG_RUNTIME_DIR", self.path.parent().unwrap())
            // This peer models IPC, not Niri's parser. Native parser checks
            // above and the real desktop fixture validate the actual syntax.
            .env("CANOPA_NIRI_BIN", "/usr/bin/true")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }
    fn send_request(&self, paths: &Paths, input: &mut std::process::ChildStdin) {
        let mut monitor = Monitor::from_output(&self.output.lock().unwrap());
        monitor.scale = 1.5;
        monitor.mode = Some(1);
        monitor.x = 120;
        serde_json::to_writer(
            &mut *input,
            &TrialRequest {
                directory: paths.directory.clone(),
                monitors: vec![monitor],
            },
        )
        .unwrap();
        input.write_all(b"\n").unwrap();
        input.flush().unwrap();
    }
    fn trial(
        &self,
        paths: &Paths,
    ) -> (
        std::process::Child,
        std::process::ChildStdin,
        BufReader<std::process::ChildStdout>,
    ) {
        let mut child = self.start_guard(paths);
        let mut input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        self.send_request(paths, &mut input);
        (child, input, output)
    }
}
#[test]
fn gui_disappearing_before_readiness_rolls_back_without_a_broken_pipe_panic() {
    let root = tempfile::tempdir().unwrap();
    let paths = paths(root.path());
    let peer = Peer::start(root.path());
    let before = serde_json::to_value(&*peer.output.lock().unwrap()).unwrap();
    let source = std::fs::read(paths.displays()).unwrap();
    let mut child = peer.start_guard(&paths);
    let mut input = child.stdin.take().unwrap();
    // Close the reader before sending any request: readiness cannot race this.
    drop(child.stdout.take().unwrap());
    peer.send_request(&paths, &mut input);
    drop(input);
    wait(&mut child);
    assert_eq!(
        serde_json::to_value(&*peer.output.lock().unwrap()).unwrap(),
        before
    );
    assert_eq!(std::fs::read(paths.displays()).unwrap(), source);
    assert_eq!(child.wait().unwrap().code(), Some(1));
}
mod output {
    pub fn logical() -> niri_ipc::LogicalOutput {
        super::output().logical.unwrap()
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.join.take().unwrap().join().unwrap();
    }
}
fn reply(reader: &mut BufReader<std::process::ChildStdout>) -> serde_json::Value {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    serde_json::from_str(&line).unwrap()
}
fn wait(child: &mut std::process::Child) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "display helper stuck");
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn gui_disconnect_restores_runtime_and_never_saves_the_trial() {
    let root = tempfile::tempdir().unwrap();
    let paths = paths(root.path());
    let peer = Peer::start(root.path());
    let original = std::fs::read(paths.displays()).unwrap();
    let (mut child, input, mut output) = peer.trial(&paths);
    let ready = reply(&mut output);
    assert_eq!(ready["state"], "ready", "{ready}");
    assert_eq!(peer.output.lock().unwrap().logical.unwrap().scale, 1.5);
    drop(input);
    assert_eq!(reply(&mut output)["state"], "reverted");
    wait(&mut child);
    assert_eq!(peer.output.lock().unwrap().logical.unwrap().scale, 1.);
    assert_eq!(peer.output.lock().unwrap().current_mode, Some(0));
    assert_eq!(std::fs::read(paths.displays()).unwrap(), original);
}
#[test]
fn explicit_keep_validates_and_persists_but_concurrent_trials_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let paths = paths(root.path());
    let peer = Peer::start(root.path());
    let (mut first, mut input, mut output) = peer.trial(&paths);
    let ready = reply(&mut output);
    assert_eq!(ready["state"], "ready", "{ready}");
    let (mut second, other, mut rejected) = peer.trial(&paths);
    assert_eq!(reply(&mut rejected)["state"], "error");
    drop(other);
    wait(&mut second);
    input.write_all(b"keep\n").unwrap();
    assert_eq!(reply(&mut output)["state"], "kept");
    wait(&mut first);
    let text = std::fs::read_to_string(paths.displays()).unwrap();
    assert!(text.contains("scale 1.5"));
    assert!(text.contains("1024x768@60.000"));
    kdl::KdlDocument::parse_v1(&text).unwrap();
}
#[test]
fn partial_apply_failure_restores_earlier_changes() {
    let root = tempfile::tempdir().unwrap();
    let paths = paths(root.path());
    let peer = Peer::start(root.path());
    peer.fail_scale.store(true, Ordering::Relaxed);
    let (mut child, input, mut output) = peer.trial(&paths);
    assert_eq!(reply(&mut output)["state"], "error");
    drop(input);
    wait(&mut child);
    assert_eq!(peer.output.lock().unwrap().current_mode, Some(0));
    assert_eq!(peer.output.lock().unwrap().logical.unwrap().scale, 1.);
}
#[test]
fn external_display_changes_during_a_trial_are_preserved() {
    let root = tempfile::tempdir().unwrap();
    let paths = paths(root.path());
    let peer = Peer::start(root.path());
    let (mut child, input, mut output) = peer.trial(&paths);
    assert_eq!(reply(&mut output)["state"], "ready");
    peer.output.lock().unwrap().logical.as_mut().unwrap().scale = 2.;
    drop(input);
    let result = reply(&mut output);
    assert_eq!(result["state"], "error");
    wait(&mut child);
    assert!(result["message"].as_str().unwrap().contains("preserved"));
    assert_eq!(peer.output.lock().unwrap().logical.unwrap().scale, 2.);
    assert!(
        !std::fs::read_to_string(paths.displays())
            .unwrap()
            .contains("scale 1.5")
    );
}
#[test]
fn timeout_reverts_without_a_gui_decision_and_compositor_identity_is_bound() {
    let root = tempfile::tempdir().unwrap();
    let paths = paths(root.path());
    let peer = Peer::start(root.path());
    let (mut child, _input, mut output) = peer.trial(&paths);
    let ready = reply(&mut output);
    assert_eq!(ready["state"], "ready", "{ready}");
    assert_eq!(reply(&mut output)["state"], "reverted");
    wait(&mut child);
    assert_eq!(peer.output.lock().unwrap().logical.unwrap().scale, 1.);
    let client = Client::connect(peer.path.clone()).unwrap();
    std::fs::remove_file(&peer.path).unwrap();
    let _other = UnixListener::bind(&peer.path).unwrap();
    assert!(
        client
            .outputs()
            .unwrap_err()
            .to_string()
            .contains("restarted")
    );
}

#[test]
fn display_profiles_round_trip_without_connector_lock_in_or_overwriting_edits() {
    let root = tempfile::tempdir().unwrap();
    let paths = paths(root.path());
    std::fs::write(paths.profiles(), "// retained comment\nstartup #false\n").unwrap();
    let book = Book::load(&paths).unwrap();
    let mut original = output();
    let mut layout = Monitor::from_output(&original);
    layout.mode = Some(1);
    layout.scale = 1.5;
    layout.transform = Transform::_90;
    layout.x = -128;
    let saved = book
        .save_layout(
            "Docked",
            &[layout],
            &HashMap::from([(original.name.clone(), original.clone())]),
        )
        .unwrap();
    assert!(saved.source.text.contains("retained comment"));
    assert!(!saved.startup);
    assert!(book.set_switching(Switching::Manual).is_err());

    original.name = "DP-6".into();
    let connected = HashMap::from([(original.name.clone(), original)]);
    let resolved = saved.get("Docked").unwrap().resolve(&connected).unwrap();
    assert_eq!(resolved[0].name, "DP-6");
    assert_eq!(resolved[0].mode, Some(1));
    assert_eq!(resolved[0].scale, 1.5);
    assert_eq!(resolved[0].transform, Transform::_90);
    assert_eq!(resolved[0].x, -128);
    assert!(saved.save_layout("Docked", &resolved, &connected).is_err());
    let configured = saved.set_switching(Switching::Automatic).unwrap();
    assert_eq!(configured.switching, Switching::Automatic);
    assert!(configured.source.text.contains("retained comment"));
    assert!(configured.remove("Docked").unwrap().profiles.is_empty());
}

#[test]
fn matching_requires_a_complete_unique_assignment_and_respects_hotplug_priority() {
    let root = tempfile::tempdir().unwrap();
    let paths = paths(root.path());
    let mut left = output();
    left.name = "DP-1".into();
    let mut right = left.clone();
    right.name = "DP-2".into();
    let connected = HashMap::from([(left.name.clone(), left), (right.name.clone(), right)]);
    std::fs::write(
        paths.profiles(),
        r#"
profile "General" {
    output connector="*"
    output connector="DP-2" enabled=#false
}
profile "Priority" priority=5 {
    output connector="DP-1"
    output connector="DP-2"
}
profile "Manual" priority=10 hotplug=#false {
    output connector="DP-1"
    output connector="DP-2"
}
profile "Ambiguous" {
    output connector="*"
    output connector="*"
}
"#,
    )
    .unwrap();
    let book = Book::load(&paths).unwrap();
    assert_eq!(
        book.matching(&connected).unwrap().unwrap().0.name,
        "Priority"
    );
    let general = book.get("General").unwrap().resolve(&connected).unwrap();
    assert_eq!(general[0].name, "DP-1");
    assert!(!general[1].enabled);
    assert!(
        book.get("Ambiguous")
            .unwrap()
            .resolve(&connected)
            .unwrap_err()
            .to_string()
            .contains("ambiguous")
    );
    let single = HashMap::from([("DP-1".into(), connected["DP-1"].clone())]);
    assert!(book.matching(&single).unwrap().is_none());

    let tied = book
        .source
        .text
        .replace("priority=10 hotplug=#false", "priority=5");
    std::fs::write(paths.profiles(), tied).unwrap();
    assert!(Book::load(&paths).unwrap().matching(&connected).is_err());
}

#[test]
fn profile_guard_keeps_live_layouts_separate_and_rolls_back_stale_automatic_changes() {
    let root = tempfile::tempdir().unwrap();
    let paths = paths(root.path());
    let peer = Peer::start(root.path());
    let baseline = std::fs::read(paths.displays()).unwrap();
    let profile = "profile \"Desk\" { output connector=\"TEST-1\" scale=1.5; }\n";
    std::fs::write(paths.profiles(), profile).unwrap();

    let start = |args: &[&str]| {
        let outputs = Client::connect(peer.path.clone())
            .unwrap()
            .outputs()
            .unwrap();
        let book = Book::load(&paths).unwrap();
        let monitors = book.get("Desk").unwrap().resolve(&outputs).unwrap();
        let mut child = peer.start_guard_with(&paths, args);
        let mut input = child.stdin.take().unwrap();
        let reader = BufReader::new(child.stdout.take().unwrap());
        serde_json::to_writer(
            &mut input,
            &TrialRequest {
                directory: paths.directory.clone(),
                monitors,
            },
        )
        .unwrap();
        input.write_all(b"\n").unwrap();
        (child, input, reader)
    };
    let (mut child, _, mut reader) = start(&["--profile=Desk", "--automatic"]);
    let error = reply(&mut reader);
    assert_eq!(error["state"], "error");
    assert!(error["message"].as_str().unwrap().contains("disabled"));
    wait(&mut child);

    let (mut child, mut input, mut reader) = start(&["--profile=Desk"]);
    assert_eq!(reply(&mut reader)["state"], "ready");
    input.write_all(b"keep\n").unwrap();
    assert_eq!(reply(&mut reader)["state"], "kept");
    wait(&mut child);
    assert_eq!(peer.output.lock().unwrap().logical.unwrap().scale, 1.5);
    assert_eq!(std::fs::read(paths.displays()).unwrap(), baseline);

    let modified = profile.replace("1.5", "1.25");
    std::fs::write(
        paths.profiles(),
        format!("switching \"automatic\"\n{modified}"),
    )
    .unwrap();
    let (mut child, mut input, mut reader) = start(&["--profile=Desk", "--automatic"]);
    assert_eq!(reply(&mut reader)["state"], "ready");
    std::fs::write(paths.profiles(), &modified).unwrap();
    input.write_all(b"keep\n").unwrap();
    assert_eq!(reply(&mut reader)["state"], "error");
    wait(&mut child);
    assert_eq!(peer.output.lock().unwrap().logical.unwrap().scale, 1.5);
    assert_eq!(std::fs::read(paths.displays()).unwrap(), baseline);
}
