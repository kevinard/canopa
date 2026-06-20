use assert_cmd::Command;
use canopa_notify::{Daemon, DaemonOptions, NotificationInput};
use predicates::str::contains;
use std::io::{BufRead, BufReader};
use std::process::{Command as StdCommand, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn panel_provider_returns_provider_visible_error_when_daemon_is_unavailable() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("missing.sock");

    Command::cargo_bin("canopa-notify")
        .unwrap()
        .args(["panel-provider", "--socket", socket.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("Canopa Notify daemon unavailable"))
        .stdout(contains("\"protocol_version\":1"));
}

#[tokio::test]
async fn panel_provider_renders_grouped_notification_history_from_daemon_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");

    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    seed.receive_notification(NotificationInput {
        app_name: "Mail".to_string(),
        summary: "Inbox".to_string(),
        body: "One unread message".to_string(),
        ..NotificationInput::default()
    })
    .await
    .unwrap();
    drop(seed);

    let mut daemon = StdCommand::new(assert_cmd::cargo::cargo_bin("canopa-notify"))
        .args([
            "daemon",
            "--headless",
            "--socket",
            socket.to_str().unwrap(),
            "--database",
            database.to_str().unwrap(),
        ])
        .spawn()
        .unwrap();

    wait_for_status(&socket);

    let mut provider = StdCommand::new(assert_cmd::cargo::cargo_bin("canopa-notify"))
        .args(["panel-provider", "--socket", socket.to_str().unwrap()])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let line = read_provider_line(&mut provider);

    assert!(line.contains("\"app_identity\":\"Mail\""));
    assert!(line.contains("\"summary\":\"Inbox\""));
    assert!(line.contains("\"protocol_version\":1"));

    provider.kill().ok();
    provider.wait().ok();
    daemon.kill().ok();
    daemon.wait().ok();
}

fn read_provider_line(provider: &mut std::process::Child) -> String {
    let stdout = provider.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let result = reader.read_line(&mut line).map(|_| line);
        sender.send(result).ok();
    });

    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("panel provider did not emit an initial projection")
        .unwrap()
}

fn wait_for_status(socket: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let assertion = Command::cargo_bin("canopa-notify")
            .unwrap()
            .args(["ctl", "--socket", socket.to_str().unwrap(), "status"])
            .assert();
        if assertion.get_output().status.success() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("daemon did not become available");
}
