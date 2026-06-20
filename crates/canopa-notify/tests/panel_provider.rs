use assert_cmd::Command;
use canopa_notify::{Daemon, DaemonOptions, NotificationInput};
use predicates::str::contains;
use std::process::Command as StdCommand;
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

    Command::cargo_bin("canopa-notify")
        .unwrap()
        .args(["panel-provider", "--socket", socket.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("\"app_identity\":\"Mail\""))
        .stdout(contains("\"summary\":\"Inbox\""))
        .stdout(contains("\"protocol_version\":1"));

    daemon.kill().ok();
    daemon.wait().ok();
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
