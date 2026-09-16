#![allow(
    dead_code,
    reason = "Each integration test binary imports only the shared helpers it needs"
)]

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
    let mut args: Vec<_> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();
    // Tests must never open the user's default persistent database.
    if args.first().is_some_and(|arg| arg == "daemon")
        && !args.iter().any(|arg| arg == "--database")
        && let Some(index) = args.iter().position(|arg| arg == "--socket")
    {
        let database = PathBuf::from(&args[index + 1]).with_extension("db");
        args.extend(["--database".into(), database.into_os_string()]);
    }
    let mut command = canopa_notify_command();
    if let Some(index) = args.iter().position(|arg| arg == "--socket") {
        let socket = PathBuf::from(&args[index + 1]);
        command.env(
            "XDG_CONFIG_HOME",
            socket.parent().unwrap().join("test-config"),
        );
    }
    command.args(args).spawn().unwrap()
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
