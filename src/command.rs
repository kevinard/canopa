//! Centralized external process execution helpers.
//!
//! This is the app's process-launch audit point. Configured script polling and
//! button clicks intentionally use `CommandSpec::Shell` to preserve existing
//! shell-based config syntax. Internal app commands, such as `wpctl` volume
//! adjustment, must use `CommandSpec::Argv` so fixed program/argument boundaries
//! stay explicit.

pub const MAX_COMMAND_OUTPUT_BYTES: usize = 4096;

use tokio::io::AsyncReadExt;

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

fn tokio_command(spec: CommandSpec<'_>) -> tokio::process::Command {
    match spec {
        CommandSpec::Shell(command) => {
            let mut child = tokio::process::Command::new("sh");
            child.arg("-c").arg(command);
            child
        }
        CommandSpec::Argv { program, args } => {
            let mut child = tokio::process::Command::new(program);
            child.args(args);
            child
        }
    }
}

fn std_command(spec: CommandSpec<'_>) -> std::process::Command {
    match spec {
        CommandSpec::Shell(command) => {
            let mut child = std::process::Command::new("sh");
            child.arg("-c").arg(command);
            child
        }
        CommandSpec::Argv { program, args } => {
            let mut child = std::process::Command::new(program);
            child.args(args);
            child
        }
    }
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
    let mut child = child.spawn()?;
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

/// Spawn a command detached. An OS thread is created per call to reap the
/// child. Intended for user-initiated actions (button clicks, keybinds) where
/// invocation rate is low; do not call in tight loops.
pub fn spawn_command_fire_and_forget(spec: CommandSpec<'_>) -> std::io::Result<SpawnResult> {
    let mut child = std_command(spec).spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(SpawnResult::Spawned)
}

async fn run_command_status(spec: CommandSpec<'_>) -> std::io::Result<std::process::ExitStatus> {
    tokio_command(spec).status().await
}

fn default_sink_volume_args(arg: &str) -> [&str; 5] {
    ["set-volume", "-l", "1", "@DEFAULT_AUDIO_SINK@", arg]
}

pub async fn set_default_sink_volume(arg: &str) -> std::io::Result<std::process::ExitStatus> {
    let args = default_sink_volume_args(arg);
    run_command_status(CommandSpec::Argv {
        program: "wpctl",
        args: &args,
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
    fn spawn_command_fire_and_forget_returns_spawned_for_argv() {
        let args: [&str; 0] = [];

        assert_eq!(
            spawn_command_fire_and_forget(CommandSpec::Argv {
                program: "true",
                args: &args,
            })
            .unwrap(),
            SpawnResult::Spawned
        );
    }

    #[test]
    fn default_sink_volume_args_keep_wpctl_on_argv_path() {
        assert_eq!(
            default_sink_volume_args("5%+"),
            ["set-volume", "-l", "1", "@DEFAULT_AUDIO_SINK@", "5%+"]
        );
    }
}
