//! Independent private authority plus native-session test double; no host authorization.
use canopa_polkit::native::{Event, Runtime};
use canopa_session_ui::secret::Secret;
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn wait(path: &Path) {
    let start = Instant::now();
    while !path.exists() {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "fixture did not start"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn native_registration_conversations_cancellation_and_owner_filter() {
    let root = tempfile::tempdir().unwrap();
    let library = root.path().join("session.so");
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/desktop-fixture/polkit-session.c");
    let flags = Command::new("pkg-config")
        .args(["--cflags", "--libs", "polkit-agent-1"])
        .output()
        .unwrap();
    assert!(flags.status.success());
    assert!(
        Command::new("cc")
            .args([
                "-shared",
                "-fPIC",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-Wno-misleading-indentation"
            ])
            .arg(source)
            .arg("-o")
            .arg(&library)
            .args(String::from_utf8(flags.stdout).unwrap().split_whitespace())
            .status()
            .unwrap()
            .success()
    );
    for scenario in [
        "otp",
        "retry",
        "deny",
        "switch",
        "cancel",
        "authority-cancel",
        "device",
        "oversized",
        "overlap",
    ] {
        let case = root.path().join(scenario);
        fs::create_dir(&case).unwrap();
        let mut bus = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut address = String::new();
        BufReader::new(bus.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let _bus = Process(bus);
        let address = address.trim();
        let mode = match scenario {
            "otp" => "otp",
            "device" | "overlap" => "device",
            "cancel" | "authority-cancel" => "late",
            _ => "password",
        };
        let mut control = json!({"auto":true,"mode":mode,"uids":if scenario=="switch" {vec![0,unsafe{libc::getuid()}]} else {vec![unsafe{libc::getuid()}]},"foreign":true});
        if scenario == "oversized" {
            control["message"] = Value::String("x".repeat(513));
        }
        fs::write(case.join("polkit-control.json"), control.to_string()).unwrap();
        let peer =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/desktop-fixture/polkit.py");
        let _peer = Process(
            Command::new("/usr/bin/python3")
                .arg(peer)
                .arg(&case)
                .env("DBUS_SESSION_BUS_ADDRESS", address)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        wait(&case.join("polkit-ready"));
        let mut child = assert_cmd::Command::new(std::env::current_exe().unwrap());
        child
            .args(["--ignored", "--exact", "native_child", "--nocapture"])
            .env("CANOPA_POLKIT_CASE", scenario)
            .env("CANOPA_POLKIT_FIXTURE_ROOT", &case)
            .env("LD_PRELOAD", &library)
            .env("DBUS_SYSTEM_BUS_ADDRESS", address)
            .env("DBUS_SESSION_BUS_ADDRESS", address)
            .env_remove("POLKIT_DEBUG")
            .timeout(Duration::from_secs(15))
            .assert()
            .success();
        let events: Vec<Value> =
            serde_json::from_slice(&fs::read(case.join("polkit-events.json")).unwrap()).unwrap();
        assert!(
            events
                .iter()
                .any(|e| e["event"] == "foreign" && e["denied"] == true),
            "{scenario}: {events:?}"
        );
        let done: Vec<_> = events.iter().filter(|e| e["event"] == "done").collect();
        assert!(
            events
                .iter()
                .any(|e| e["event"] == "foreign-no-interface" && e["denied"] == true),
            "{scenario}: {events:?}"
        );
        assert!(!done.is_empty(), "{scenario}");
        if ["cancel", "authority-cancel", "oversized", "deny"].contains(&scenario) {
            assert!(done.iter().all(|e| e["authorized"] == false));
            assert!(!events.iter().any(|e| e["event"] == "receipt"));
        } else {
            assert!(
                done.iter().any(|e| e["authorized"] == true),
                "{scenario}: {events:?}"
            );
        }
        if scenario == "deny" {
            assert!(done.iter().all(|e| e["error"] == ""));
        }
        if scenario == "switch" {
            assert!(
                events
                    .iter()
                    .any(|e| e["event"] == "receipt" && e["uid"] == 0)
            );
        }
        if scenario == "overlap" {
            assert_eq!(done.len(), 2);
            assert_eq!(done.iter().filter(|e| e["authorized"] == false).count(), 1);
        }
    }
}
#[test]
#[ignore = "test-owned child with private bus and preload fixture"]
fn native_child() {
    let scenario = std::env::var("CANOPA_POLKIT_CASE").unwrap();
    let root = std::path::PathBuf::from(std::env::var_os("CANOPA_POLKIT_FIXTURE_ROOT").unwrap());
    let (sender, receiver) = calloop::channel::channel();
    let runtime = Runtime::start("canopa-fixture", 10, sender).unwrap();
    let start = Instant::now();
    let mut count = 0;
    let mut last_serial = 0;
    let mut retry = None;
    let mut switched = false;
    let mut closed = false;
    let mut records = Vec::new();
    loop {
        assert!(
            start.elapsed() < Duration::from_secs(12),
            "native scenario timed out: {scenario}, {records:?}"
        );
        if let Some((deadline, generation, serial)) = retry
            && Instant::now() >= deadline
        {
            Runtime::command(generation, serial, 2, 0, None);
            retry = None;
        }
        while let Ok(event) = receiver.try_recv() {
            match event {
                Event::Ready => {
                    records.push("ready");
                    let (other, _) = calloop::channel::channel();
                    assert!(Runtime::start("canopa-fixture", 5, other).is_err());
                }
                Event::Begin { .. } => records.push("begin"),
                Event::Prompt {
                    generation,
                    serial,
                    echo,
                    ..
                } => {
                    count += 1;
                    last_serial = serial;
                    records.push(if echo {
                        "visible-prompt"
                    } else {
                        "secret-prompt"
                    });
                    if scenario == "cancel" {
                        Runtime::command(generation, serial, 1, 0, None);
                    } else if scenario == "authority-cancel" {
                        fs::write(root.join("polkit-cancel"), b"cancel").unwrap();
                    } else if scenario == "switch" && !switched {
                        switched = true;
                        Runtime::command(generation, serial, 2, 99, None);
                        Runtime::command(generation, serial, 2, 0, None);
                        let mut stale = Secret::default();
                        stale.push("stale-password-must-not-be-used");
                        Runtime::command(generation, serial, 3, 0, Some(&stale));
                    } else {
                        let mut secret = Secret::default();
                        secret.push(
                            if scenario == "deny" || (scenario == "retry" && count == 1) {
                                "wrong-password"
                            } else if echo {
                                "123456"
                            } else {
                                "canopa-fixture-password"
                            },
                        );
                        Runtime::command(generation, serial, 3, 0, Some(&secret));
                    }
                }
                Event::Failed { generation } => {
                    records.push("failed");
                    retry = Some((
                        Instant::now() + Duration::from_millis(2100),
                        generation,
                        last_serial,
                    ));
                }
                Event::Info { .. } => {
                    records.push("notice");
                    if scenario == "device" {
                        fs::write(root.join("polkit-touch"), b"touch").unwrap();
                    }
                    if scenario == "overlap" {
                        fs::write(root.join("polkit-overlap"), b"overlap").unwrap();
                    }
                }
                Event::Closed { .. } => {
                    records.push("closed");
                    closed = true;
                }
                Event::Fatal | Event::Stopped(_) => panic!("native runtime failed"),
                _ => {}
            }
        }
        if scenario == "overlap" && root.join("polkit-session-events").exists() {
            let events = fs::read_to_string(root.join("polkit-events.json")).unwrap_or_default();
            if events.contains("done") {
                fs::write(root.join("polkit-touch"), b"touch").unwrap();
            }
        }
        let events: Vec<Value> = fs::read(root.join("polkit-events.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        if (closed || scenario == "oversized")
            && events.iter().filter(|e| e["event"] == "done").count()
                >= if scenario == "overlap" { 2 } else { 1 }
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    if scenario == "otp" {
        assert_eq!(count, 2);
        assert!(records.contains(&"visible-prompt"));
    }
    if scenario == "device" {
        assert_eq!(count, 0);
    }
    if scenario == "oversized" {
        assert!(!records.contains(&"begin"));
    }
    if scenario == "cancel" || scenario == "authority-cancel" {
        std::thread::sleep(Duration::from_millis(150));
    }
    drop(runtime);
    fs::write(
        root.join("native-events.json"),
        serde_json::to_vec(&records).unwrap(),
    )
    .unwrap();
}
