use assert_cmd::Command;
use predicates::str::contains;
use std::process::Child;
use std::process::Command as StdCommand;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn ctl_status_reports_clear_unavailable_error_when_daemon_is_not_running() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");

    Command::cargo_bin("canopa-notify")
        .unwrap()
        .args(["ctl", "--socket", socket.to_str().unwrap(), "status"])
        .assert()
        .failure()
        .stderr(contains("Canopa Notify daemon unavailable"));
}

#[test]
fn headless_daemon_reports_available_through_private_runtime_socket() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");

    let mut daemon = StdCommand::new(assert_cmd::cargo::cargo_bin("canopa-notify"))
        .args(["daemon", "--headless", "--socket", socket.to_str().unwrap()])
        .spawn()
        .unwrap();

    wait_for_status(&socket, &mut daemon);

    Command::cargo_bin("canopa-notify")
        .unwrap()
        .args(["ctl", "--socket", socket.to_str().unwrap(), "status"])
        .assert()
        .success()
        .stdout(contains("Canopa Notify daemon available"));

    daemon.kill().ok();
    daemon.wait().ok();
}

#[test]
fn daemon_fails_clearly_when_another_instance_owns_the_runtime_socket() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");

    let mut first = StdCommand::new(assert_cmd::cargo::cargo_bin("canopa-notify"))
        .args(["daemon", "--headless", "--socket", socket.to_str().unwrap()])
        .spawn()
        .unwrap();
    wait_for_status(&socket, &mut first);

    Command::cargo_bin("canopa-notify")
        .unwrap()
        .args(["daemon", "--headless", "--socket", socket.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("Canopa Notify daemon already running"));

    first.kill().ok();
    first.wait().ok();
}

fn wait_for_status(socket: &std::path::Path, daemon: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Some(status) = daemon.try_wait().unwrap() {
            panic!("daemon exited before status was available: {status}");
        }

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
