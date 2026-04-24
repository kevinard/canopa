//! Centralized shell-command execution helpers. Single audit point for
//! spawning external processes.

pub const MAX_COMMAND_OUTPUT_BYTES: usize = 4096;

use tokio::io::AsyncReadExt;

#[derive(Debug, PartialEq, Eq)]
pub enum CaptureResult {
    Success(String),
    NonZero(std::process::ExitStatus),
    TimedOut,
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

/// Run `sh -c <command>` with a timeout and capture stdout.
///
/// Returns:
/// - `Ok(CaptureResult::Success(output))` on zero exit — stdout trimmed and byte-capped at `max_output_bytes`.
/// - `Ok(CaptureResult::NonZero(status))` on non-zero exit.
/// - `Ok(CaptureResult::TimedOut)` on timeout.
/// - `Err(e)` only on spawn-level I/O errors.
///
/// Timeout cancellation kills and waits on the child so the process is reaped.
pub async fn run_shell_capture_stdout(
    command: &str,
    timeout: std::time::Duration,
    max_output_bytes: usize,
) -> std::io::Result<CaptureResult> {
    let mut child = tokio::process::Command::new("sh");
    child
        .arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::piped());
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

/// Spawn `sh -c <command>` detached. An OS thread is created per call to
/// reap the child. Intended for user-initiated actions (button clicks,
/// keybinds) where invocation rate is low; do not call in tight loops.
pub fn spawn_shell_fire_and_forget(command: &str) -> std::io::Result<()> {
    let mut child = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

pub async fn set_default_sink_volume(arg: &str) -> std::io::Result<std::process::ExitStatus> {
    tokio::process::Command::new("wpctl")
        .args(["set-volume", "-l", "1", "@DEFAULT_AUDIO_SINK@", arg])
        .status()
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
    async fn run_shell_capture_stdout_returns_success_on_zero_exit() {
        let out = run_shell_capture_stdout(
            "printf hello",
            Duration::from_secs(1),
            MAX_COMMAND_OUTPUT_BYTES,
        )
        .await
        .unwrap();
        assert_eq!(out, CaptureResult::Success("hello".to_string()));
    }

    #[tokio::test]
    async fn run_shell_capture_stdout_returns_nonzero_on_nonzero_exit() {
        let out =
            run_shell_capture_stdout("false", Duration::from_secs(1), MAX_COMMAND_OUTPUT_BYTES)
                .await
                .unwrap();
        assert!(matches!(out, CaptureResult::NonZero(status) if !status.success()));
    }

    #[tokio::test]
    async fn run_shell_capture_stdout_returns_timed_out_on_timeout() {
        let out = run_shell_capture_stdout(
            "sleep 1",
            Duration::from_millis(50),
            MAX_COMMAND_OUTPUT_BYTES,
        )
        .await
        .unwrap();
        assert_eq!(out, CaptureResult::TimedOut);
    }

    #[tokio::test]
    async fn run_shell_capture_stdout_caps_large_output_without_timeout() {
        let out = run_shell_capture_stdout(
            "yes x | tr -d '\n' | head -c 8192",
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
    async fn run_shell_capture_stdout_drains_after_cap() {
        let out = run_shell_capture_stdout(
            "head -c 131072 /dev/zero | tr '\\0' x",
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
}
