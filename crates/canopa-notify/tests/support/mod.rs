use assert_cmd::Command;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as StdCommand};
use std::thread;
use std::time::{Duration, Instant};

pub mod notifications;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub fn path_arg(path: &Path) -> &str {
    path.to_str().expect("test path must be valid UTF-8")
}

pub fn canopa_notify_command() -> StdCommand {
    StdCommand::new(canopa_notify_bin())
}

pub fn spawn_canopa_notify<I, S>(args: I) -> Child
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    canopa_notify_command().args(args).spawn().unwrap()
}

pub fn wait_for_status_with_child(socket: &Path, daemon: &mut Child) {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        if let Some(status) = daemon.try_wait().unwrap() {
            panic!("daemon exited before status was available: {status}");
        }

        let assertion = Command::cargo_bin("canopa-notify")
            .unwrap()
            .args(["ctl", "--socket", path_arg(socket), "status"])
            .assert();

        if assertion.get_output().status.success() {
            return;
        }

        thread::sleep(STATUS_POLL_INTERVAL);
    }

    panic!("daemon did not become available");
}

fn canopa_notify_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin("canopa-notify")
}
