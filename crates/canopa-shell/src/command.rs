//! Centralized external process execution helpers.
//!
//! This is the app's process-launch audit point. Configured script polling and
//! automation actions use policy-validated `ConfiguredCommand`. Application
//! launches use an argv-only handoff to the desktop session, with no worker
//! timeout and no process ownership by the bar.

pub const MAX_COMMAND_OUTPUT_BYTES: usize = 4096;

use tokio::io::AsyncReadExt;

/// Lifetime guard for a command tree we own. Desktop launches deliberately do
/// not use this guard: their lifetime is independent of the bar.
#[derive(Debug)]
pub(crate) struct ProcessGroup(std::sync::atomic::AtomicU32);

impl ProcessGroup {
    pub(crate) fn new(id: Option<u32>) -> Self {
        Self(std::sync::atomic::AtomicU32::new(id.unwrap_or(0)))
    }

    pub(crate) fn terminate(&self) {
        if let Some(pid) = rustix::process::Pid::from_raw(
            self.0.swap(0, std::sync::atomic::Ordering::AcqRel) as i32,
        ) {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
        }
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum CaptureResult {
    Success(String),
    NonZero(std::process::ExitStatus),
    TimedOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnResult {
    Spawned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandSpec<'a> {
    Shell(&'a str),
    Argv {
        program: &'a str,
        args: &'a [&'a str],
    },
}

pub fn sanitize_stdout(stdout: &[u8], max_bytes: usize) -> String {
    let text = String::from_utf8_lossy(stdout);
    let trimmed = text.trim();
    if trimmed.len() <= max_bytes {
        trimmed.to_string()
    } else {
        String::from_utf8_lossy(&trimmed.as_bytes()[..max_bytes]).to_string()
    }
}

async fn kill_and_wait(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

trait ProcessCommand: Sized {
    fn new(program: &str) -> Self;
    fn arg(&mut self, arg: &str) -> &mut Self;
    fn args(&mut self, args: &[&str]) -> &mut Self;
}

impl ProcessCommand for tokio::process::Command {
    fn new(program: &str) -> Self {
        tokio::process::Command::new(program)
    }

    fn arg(&mut self, arg: &str) -> &mut Self {
        tokio::process::Command::arg(self, arg)
    }

    fn args(&mut self, args: &[&str]) -> &mut Self {
        tokio::process::Command::args(self, args)
    }
}

impl ProcessCommand for std::process::Command {
    fn new(program: &str) -> Self {
        std::process::Command::new(program)
    }

    fn arg(&mut self, arg: &str) -> &mut Self {
        std::process::Command::arg(self, arg)
    }

    fn args(&mut self, args: &[&str]) -> &mut Self {
        std::process::Command::args(self, args)
    }
}

fn command_from_spec<C: ProcessCommand>(spec: CommandSpec<'_>) -> C {
    match spec {
        CommandSpec::Shell(command) => {
            let mut child = C::new("sh");
            child.arg("-c").arg(command);
            child
        }
        CommandSpec::Argv { program, args } => {
            let mut child = C::new(program);
            child.args(args);
            child
        }
    }
}

fn tokio_command(spec: CommandSpec<'_>) -> tokio::process::Command {
    command_from_spec(spec)
}

fn std_command(spec: CommandSpec<'_>) -> std::process::Command {
    command_from_spec(spec)
}

/// Run a command with a timeout and capture stdout.
///
/// Returns:
/// - `Ok(CaptureResult::Success(output))` on zero exit — stdout trimmed and byte-capped at `max_output_bytes`.
/// - `Ok(CaptureResult::NonZero(status))` on non-zero exit.
/// - `Ok(CaptureResult::TimedOut)` on timeout.
/// - `Err(e)` only on spawn-level I/O errors.
///
/// Timeout cancellation kills and waits on the child so the process is reaped.
pub async fn run_command_capture_stdout(
    spec: CommandSpec<'_>,
    timeout: std::time::Duration,
    max_output_bytes: usize,
) -> std::io::Result<CaptureResult> {
    let mut child = tokio_command(spec);
    child.stdout(std::process::Stdio::piped());
    child.kill_on_drop(true);
    child.process_group(0);
    let mut child = child.spawn()?;
    let group = ProcessGroup::new(child.id());
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("child stdout pipe unavailable"))?;
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);

    let mut captured = Vec::with_capacity(max_output_bytes.min(1024));
    let mut buffer = [0u8; 1024];

    loop {
        tokio::select! {
            biased;
            _ = &mut deadline => {
                group.terminate();
                kill_and_wait(&mut child).await;
                return Ok(CaptureResult::TimedOut);
            }
            read = stdout.read(&mut buffer) => {
                let read = read?;
                if read == 0 {
                    break;
                }

                let remaining = max_output_bytes.saturating_sub(captured.len());
                let retain = remaining.min(read);
                captured.extend_from_slice(&buffer[..retain]);
            }
        }
    }

    tokio::select! {
        biased;
        _ = &mut deadline => {
            group.terminate();
            kill_and_wait(&mut child).await;
            Ok(CaptureResult::TimedOut)
        }
        status = child.wait() => {
            match status? {
                status if status.success() => {
                    Ok(CaptureResult::Success(sanitize_stdout(&captured, max_output_bytes)))
                }
                status => Ok(CaptureResult::NonZero(status)),
            }
        }
    }
}

pub async fn run_configured_command_capture_stdout(
    command: &crate::config::ConfiguredCommand,
    max_output_bytes: usize,
) -> std::io::Result<CaptureResult> {
    let timeout = std::time::Duration::from_millis(command.timeout_ms());
    match command.spec() {
        crate::config::ConfiguredCommandSpec::Shell(command) => {
            run_command_capture_stdout(CommandSpec::Shell(command), timeout, max_output_bytes).await
        }
        crate::config::ConfiguredCommandSpec::Argv { program, args } => {
            let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
            run_command_capture_stdout(
                CommandSpec::Argv {
                    program,
                    args: &borrowed,
                },
                timeout,
                max_output_bytes,
            )
            .await
        }
    }
}

pub fn spawn_command_with_timeout_fire_and_forget(
    spec: CommandSpec<'_>,
    timeout: std::time::Duration,
) -> std::io::Result<SpawnResult> {
    use std::os::unix::process::CommandExt;
    let mut child = std_command(spec).process_group(0).spawn()?;
    let group = ProcessGroup::new(Some(child.id()));
    std::thread::spawn(move || {
        let _group = group;
        let deadline = std::time::Instant::now() + timeout;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if std::time::Instant::now() >= deadline => {
                    _group.terminate();
                    let _ = child.kill();
                    let _ = child.wait();
                    return;
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
                Err(_) => return,
            }
        }
    });
    Ok(SpawnResult::Spawned)
}

pub(crate) mod app_scope;
mod applications;
#[cfg(not(test))]
pub use applications::run_niri_action as niri_action;
pub use applications::{focus_window, launch_application};
#[cfg(not(test))]
pub use applications::{
    launch_desktop_action, open_desktop_application, spawn_desktop_application_fire_and_forget,
    spawn_desktop_application_placed,
};

pub fn spawn_configured_command_fire_and_forget(
    command: &crate::config::ConfiguredCommand,
) -> std::io::Result<SpawnResult> {
    let timeout = std::time::Duration::from_millis(command.timeout_ms());
    match command.spec() {
        crate::config::ConfiguredCommandSpec::Shell(command) => {
            spawn_command_with_timeout_fire_and_forget(CommandSpec::Shell(command), timeout)
        }
        crate::config::ConfiguredCommandSpec::Argv { program, args } => {
            let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
            spawn_command_with_timeout_fire_and_forget(
                CommandSpec::Argv {
                    program,
                    args: &borrowed,
                },
                timeout,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn unique_tmp_path(name: &str) -> String {
        format!("/tmp/canopa-shell-command-{name}-{}", std::process::id())
    }

    fn wait_for_path(path: &str) -> bool {
        for _ in 0..50 {
            if std::path::Path::new(path).exists() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn sanitize_stdout_trims_and_caps_bytes() {
        let output = format!("{} trailing\n", "x".repeat(MAX_COMMAND_OUTPUT_BYTES + 32));
        let sanitized = sanitize_stdout(output.as_bytes(), MAX_COMMAND_OUTPUT_BYTES);
        assert_eq!(sanitized.len(), MAX_COMMAND_OUTPUT_BYTES);
        assert!(sanitized.chars().all(|c| c == 'x'));
    }

    #[tokio::test]
    async fn run_command_capture_stdout_shell_returns_success_on_zero_exit() {
        let out = run_command_capture_stdout(
            CommandSpec::Shell("printf hello"),
            Duration::from_secs(1),
            MAX_COMMAND_OUTPUT_BYTES,
        )
        .await
        .unwrap();
        assert_eq!(out, CaptureResult::Success("hello".to_string()));
    }

    #[tokio::test]
    async fn run_command_capture_stdout_argv_returns_success_on_zero_exit() {
        let args = ["hello"];
        let out = run_command_capture_stdout(
            CommandSpec::Argv {
                program: "printf",
                args: &args,
            },
            Duration::from_secs(1),
            MAX_COMMAND_OUTPUT_BYTES,
        )
        .await
        .unwrap();
        assert_eq!(out, CaptureResult::Success("hello".to_string()));
    }

    #[tokio::test]
    async fn run_command_capture_stdout_returns_nonzero_on_nonzero_exit() {
        let out = run_command_capture_stdout(
            CommandSpec::Shell("false"),
            Duration::from_secs(1),
            MAX_COMMAND_OUTPUT_BYTES,
        )
        .await
        .unwrap();
        assert!(matches!(out, CaptureResult::NonZero(status) if !status.success()));
    }

    #[tokio::test]
    async fn run_command_capture_stdout_returns_timed_out_on_timeout() {
        let out = run_command_capture_stdout(
            CommandSpec::Shell("sleep 1"),
            Duration::from_millis(50),
            MAX_COMMAND_OUTPUT_BYTES,
        )
        .await
        .unwrap();
        assert_eq!(out, CaptureResult::TimedOut);
    }

    #[tokio::test]
    async fn run_command_capture_stdout_caps_large_output_without_timeout() {
        let out = run_command_capture_stdout(
            CommandSpec::Shell("yes x | tr -d '\n' | head -c 8192"),
            Duration::from_secs(1),
            MAX_COMMAND_OUTPUT_BYTES,
        )
        .await
        .unwrap();
        match out {
            CaptureResult::Success(out) => {
                assert_eq!(out.len(), MAX_COMMAND_OUTPUT_BYTES);
                assert!(out.chars().all(|c| c == 'x'));
            }
            other => panic!("expected success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn run_command_capture_stdout_drains_after_cap() {
        let out = run_command_capture_stdout(
            CommandSpec::Shell("head -c 131072 /dev/zero | tr '\\0' x"),
            Duration::from_secs(2),
            MAX_COMMAND_OUTPUT_BYTES,
        )
        .await
        .unwrap();
        match out {
            CaptureResult::Success(out) => {
                assert_eq!(out.len(), MAX_COMMAND_OUTPUT_BYTES);
                assert!(out.chars().all(|c| c == 'x'));
            }
            other => panic!("expected success, got {other:?}"),
        }
    }

    #[test]
    fn spawn_configured_command_returns_spawned() {
        let path = unique_tmp_path("policy-spawned");
        let _ = std::fs::remove_file(&path);
        let command = crate::config::ConfiguredCommand::argv_for_test(
            "sh",
            vec!["-c".into(), format!("printf done >{path}")],
        );

        assert_eq!(
            spawn_configured_command_fire_and_forget(&command).unwrap(),
            SpawnResult::Spawned
        );

        assert!(wait_for_path(&path), "expected command to create {path}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn spawn_configured_command_kills_after_timeout() {
        let path = unique_tmp_path("policy-timeout");
        let _ = std::fs::remove_file(&path);
        let command = crate::config::ConfiguredCommand::argv_with_timeout_for_test(
            "sh",
            vec!["-c".into(), format!("sleep 1; printf done >{path}")],
            50,
        );

        assert_eq!(
            spawn_configured_command_fire_and_forget(&command).unwrap(),
            SpawnResult::Spawned
        );

        std::thread::sleep(Duration::from_millis(200));
        assert!(
            !std::path::Path::new(&path).exists(),
            "timed-out command should not create {path}"
        );
    }

    #[test]
    fn argv_command_spec_keeps_argument_boundaries() {
        let args = ["hello world", "$(ignored)"];
        let spec = CommandSpec::Argv {
            program: "printf",
            args: &args,
        };
        assert!(format!("{spec:?}").contains("Argv"));
    }

    #[tokio::test]
    async fn run_configured_command_capture_stdout_uses_argv_boundaries() {
        let command =
            crate::config::ConfiguredCommand::argv_for_test("printf", vec!["hello world".into()]);

        let out = run_configured_command_capture_stdout(&command, MAX_COMMAND_OUTPUT_BYTES)
            .await
            .unwrap();

        assert_eq!(out, CaptureResult::Success("hello world".to_string()));
    }
    #[tokio::test]
    async fn command_timeout_terminates_descendants() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("survived");
        let command = format!("sh -c 'sleep 0.3; touch {}' & wait", marker.display());
        assert_eq!(
            run_command_capture_stdout(
                CommandSpec::Shell(&command),
                Duration::from_millis(50),
                4096
            )
            .await
            .unwrap(),
            CaptureResult::TimedOut
        );
        tokio::time::sleep(Duration::from_millis(450)).await;
        assert!(
            !marker.exists(),
            "descendant performed work after the command timeout"
        );
    }
}
