use crate::support;

use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn ctl_status_reports_clear_unavailable_error_when_daemon_is_not_running() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");

    Command::cargo_bin("canopa-notify")
        .unwrap()
        .args(["ctl", "--socket", support::path_arg(&socket), "status"])
        .assert()
        .failure()
        .stderr(contains("Canopa Notify daemon unavailable"));
}

#[test]
fn headless_daemon_reports_available_through_private_runtime_socket() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");

    let mut daemon = support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
    ]);

    support::wait_for_status_with_child(&socket, &mut daemon);

    Command::cargo_bin("canopa-notify")
        .unwrap()
        .args(["ctl", "--socket", support::path_arg(&socket), "status"])
        .assert()
        .success()
        .stdout(contains("Canopa Notify daemon available"));

    let status = Command::cargo_bin("canopa-notify")
        .unwrap()
        .args([
            "ctl",
            "--socket",
            support::path_arg(&socket),
            "status",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let status: serde_json::Value = serde_json::from_slice(&status).unwrap();
    assert_eq!(status["timeout_records"], 0);

    daemon.kill().ok();
    daemon.wait().ok();
}

#[test]
fn daemon_fails_clearly_when_another_instance_owns_the_runtime_socket() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");

    let mut first = support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
    ]);
    support::wait_for_status_with_child(&socket, &mut first);

    Command::cargo_bin("canopa-notify")
        .unwrap()
        .args([
            "daemon",
            "--headless",
            "--socket",
            support::path_arg(&socket),
        ])
        .assert()
        .failure()
        .stderr(contains("Canopa Notify daemon already running"));

    first.kill().ok();
    first.wait().ok();
}

#[tokio::test]
async fn rejected_duplicate_startup_preserves_active_notification_history() {
    use canopa_notify::{Daemon, DaemonOptions, NotificationInput};
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let daemon = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    let receipt = daemon
        .receive_notification(NotificationInput {
            app_name: "Mail".into(),
            summary: "Still active".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    // A listening endpoint represents a running owner; inspect state through Daemon.
    let _owner = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    Command::cargo_bin("canopa-notify")
        .unwrap()
        .args([
            "daemon",
            "--headless",
            "--socket",
            support::path_arg(&socket),
            "--database",
            support::path_arg(&database),
        ])
        .assert()
        .failure()
        .stderr(contains("already running"));
    let snapshot = daemon.snapshot().await.unwrap();
    assert_eq!(
        snapshot.active.len(),
        1,
        "a rejected startup must not recover another daemon's history"
    );
    assert_eq!(snapshot.active[0].id, receipt.notification_id);
}

#[tokio::test]
async fn default_database_uses_persistent_state_independent_of_the_socket() {
    use canopa_notify::{Daemon, DaemonOptions, NotificationInput};
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join("state");
    let database = state.join("canopa-notify/history.db");
    let seed = Daemon::open(DaemonOptions::headless(database))
        .await
        .unwrap();
    seed.receive_notification(NotificationInput {
        app_name: "Mail".into(),
        summary: "Retained message".into(),
        ..Default::default()
    })
    .await
    .unwrap();
    let socket = temp.path().join("new-session/control.sock");
    let mut child = support::canopa_notify_command()
        .env("XDG_STATE_HOME", &state)
        .args([
            "daemon",
            "--headless",
            "--socket",
            support::path_arg(&socket),
        ])
        .spawn()
        .unwrap();
    support::wait_for_status_with_child(&socket, &mut child);
    use std::io::{Read, Write};
    let mut stream = std::os::unix::net::UnixStream::connect(&socket).unwrap();
    stream.write_all(b"panel-snapshot\n").unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    child.kill().ok();
    child.wait().ok();
    assert!(reply.contains("Retained message"), "{reply}");
}

#[test]
fn cli_do_not_disturb_survives_a_daemon_restart() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let args = [
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
    ];
    let mut daemon = support::spawn_canopa_notify(args);
    support::wait_for_status_with_child(&socket, &mut daemon);
    let enabled = Command::cargo_bin("canopa-notify")
        .unwrap()
        .args(["ctl", "--socket", support::path_arg(&socket), "dnd", "on"])
        .output()
        .unwrap();
    daemon.kill().ok();
    daemon.wait().ok();
    assert!(
        enabled.status.success(),
        "{}",
        String::from_utf8_lossy(&enabled.stderr)
    );
    let mut daemon = support::spawn_canopa_notify(args);
    support::wait_for_status_with_child(&socket, &mut daemon);
    let status = Command::cargo_bin("canopa-notify")
        .unwrap()
        .args(["ctl", "--socket", support::path_arg(&socket), "dnd"])
        .output()
        .unwrap();
    daemon.kill().ok();
    daemon.wait().ok();
    assert!(status.status.success());
    assert_eq!(String::from_utf8(status.stdout).unwrap().trim(), "on");
}

#[test]
fn oversized_control_command_is_rejected_without_disrupting_the_daemon() {
    use std::io::{Read, Write};
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let mut daemon = support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
    ]);
    support::wait_for_status_with_child(&socket, &mut daemon);
    let mut stream = std::os::unix::net::UnixStream::connect(&socket).unwrap();
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .unwrap();
    let request = format!(
        "control {{\"command\":\"set-app-permission\",\"app_identity\":\"{}\",\"permission\":\"silence\"}}\n",
        "a".repeat(70_000)
    );
    let _ = stream.write_all(request.as_bytes());
    let mut reply = String::new();
    let _ = stream.read_to_string(&mut reply);
    let status = Command::cargo_bin("canopa-notify")
        .unwrap()
        .args(["ctl", "--socket", support::path_arg(&socket), "status"])
        .output()
        .unwrap();
    daemon.kill().ok();
    daemon.wait().ok();
    assert!(
        reply.starts_with("error "),
        "oversized command was accepted"
    );
    assert!(status.status.success());
}
