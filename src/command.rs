//! Centralized external process execution helpers.
//!
//! This is the app's process-launch audit point. Configured script polling and
//! button clicks use `CommandConfig` so config can preserve existing shell
//! syntax or opt into argv boundaries. Internal app commands, such as `wpctl`
//! volume adjustment, must use `CommandSpec::Argv` so fixed
//! program/argument boundaries stay explicit.

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopLaunchArgv {
    pub program: String,
    pub args: Vec<String>,
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

pub async fn run_config_command_capture_stdout(
    config: &crate::config::CommandConfig,
    timeout: std::time::Duration,
    max_output_bytes: usize,
) -> std::io::Result<CaptureResult> {
    match config {
        crate::config::CommandConfig::Shell(command) => {
            run_command_capture_stdout(CommandSpec::Shell(command), timeout, max_output_bytes).await
        }
        crate::config::CommandConfig::Argv { program, args } => {
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

pub fn desktop_exec_to_argv(exec: &str) -> std::io::Result<DesktopLaunchArgv> {
    let tokens = split_desktop_exec(exec)?;
    let mut cleaned = Vec::new();
    for token in tokens {
        let token = strip_desktop_field_codes(&token);
        if !token.is_empty() {
            cleaned.push(token);
        }
    }
    let Some(program) = cleaned.first().cloned() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "desktop Exec produced no program",
        ));
    };
    Ok(DesktopLaunchArgv {
        program,
        args: cleaned.into_iter().skip(1).collect(),
    })
}

#[cfg_attr(test, allow(dead_code))]
pub fn spawn_desktop_application_fire_and_forget(
    app: &crate::desktop_apps::DesktopApplication,
) -> std::io::Result<SpawnResult> {
    let argv = desktop_exec_to_argv(&app.exec)?;
    let borrowed: Vec<&str> = argv.args.iter().map(String::as_str).collect();
    spawn_command_fire_and_forget(CommandSpec::Argv {
        program: &argv.program,
        args: &borrowed,
    })
}

pub fn spawn_config_command_fire_and_forget(
    config: &crate::config::CommandConfig,
) -> std::io::Result<SpawnResult> {
    match config {
        crate::config::CommandConfig::Shell(command) => {
            spawn_command_fire_and_forget(CommandSpec::Shell(command))
        }
        crate::config::CommandConfig::Argv { program, args } => {
            let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
            spawn_command_fire_and_forget(CommandSpec::Argv {
                program,
                args: &borrowed,
            })
        }
    }
}

fn split_desktop_exec(exec: &str) -> std::io::Result<Vec<String>> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut chars = exec.chars().peekable();
    let mut quote = None;

    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (None, ch) if ch.is_whitespace() => {
                if !token.is_empty() {
                    tokens.push(std::mem::take(&mut token));
                }
            }
            (None, '\'' | '"') => quote = Some(ch),
            (Some(active), ch) if ch == active => quote = None,
            (_, '\\') => {
                if let Some(next) = chars.next() {
                    token.push(next);
                }
            }
            _ => token.push(ch),
        }
    }

    if quote.is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "desktop Exec contains unterminated quote",
        ));
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    Ok(tokens)
}

fn strip_desktop_field_codes(token: &str) -> String {
    let mut cleaned = String::new();
    let mut chars = token.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            cleaned.push(ch);
            continue;
        }
        match chars.next() {
            Some('%') => cleaned.push('%'),
            Some('f' | 'F' | 'u' | 'U' | 'i' | 'c' | 'k') => {}
            Some(_) => {}
            None => cleaned.push('%'),
        }
    }
    cleaned
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
    fn desktop_exec_removes_file_field_codes_for_plain_launch() {
        let argv = desktop_exec_to_argv("firefox %U --new-window").unwrap();
        assert_eq!(argv.program, "firefox");
        assert_eq!(argv.args, vec!["--new-window"]);
    }

    #[test]
    fn desktop_exec_unescapes_literal_percent() {
        let argv = desktop_exec_to_argv("app --label 100%%").unwrap();
        assert_eq!(argv.program, "app");
        assert_eq!(argv.args, vec!["--label", "100%"]);
    }

    #[test]
    fn desktop_exec_removes_metadata_field_codes() {
        let argv = desktop_exec_to_argv("app %i %c %k").unwrap();
        assert_eq!(argv.program, "app");
        assert!(argv.args.is_empty());
    }

    #[test]
    fn desktop_exec_preserves_quoted_arguments() {
        let argv = desktop_exec_to_argv("app --name \"Two Words\"").unwrap();
        assert_eq!(argv.program, "app");
        assert_eq!(argv.args, vec!["--name", "Two Words"]);
    }

    #[test]
    fn default_sink_volume_args_keep_wpctl_on_argv_path() {
        assert_eq!(
            default_sink_volume_args("5%+"),
            ["set-volume", "-l", "1", "@DEFAULT_AUDIO_SINK@", "5%+"]
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
    async fn run_config_command_capture_stdout_uses_argv_boundaries() {
        let config = crate::config::CommandConfig::Argv {
            program: "printf".into(),
            args: vec!["hello world".into()],
        };

        let out = run_config_command_capture_stdout(
            &config,
            Duration::from_secs(1),
            MAX_COMMAND_OUTPUT_BYTES,
        )
        .await
        .unwrap();

        assert_eq!(out, CaptureResult::Success("hello world".to_string()));
    }
}
