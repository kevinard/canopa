mod support;

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
