//! Explicit compositor selection belongs to the login launcher, before Wayland connects.
//!
//! Cage and Weston run the greeter themselves and exit with it, so the launcher
//! replaces itself with them. Niri spawns its startup command detached and keeps
//! running, so for Niri the launcher stays the parent of both processes: it
//! starts Niri with a generated login configuration, learns the Wayland socket
//! from a reporter Niri spawns, runs the greeter as its own child and stops Niri
//! when the greeter exits. greetd starts the desktop only after the launcher exits,
//! by which time Niri has released the display.
use anyhow::{Context, Result, bail, ensure};
use canopa_theme::Rgb;
use clap::ValueEnum;
use std::{
    ffi::{OsStr, OsString},
    os::unix::{
        fs::{DirBuilderExt, OpenOptionsExt},
        net::UnixDatagram,
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Compositor {
    #[default]
    Niri,
    Cage,
    Weston,
}
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Renderer {
    #[default]
    Auto,
    Software,
}

/// The compositor build shipped by the Arch `canopa-compositor` package.
pub const BUNDLED_NIRI: &str = "/usr/lib/canopa/niri";
/// Display layout for the login screen, written by Settings through
/// `canopa-greeter-displays`. Niri includes it only when root owns it.
pub const LOGIN_DISPLAYS: &str = "/etc/canopa/greeter-displays.kdl";
/// Asks the Canopa Niri build to leave its last frame on screen when it exits;
/// other Niri builds ignore it.
pub const KEEP_LAST_FRAME: &str = "NIRI_KEEP_LAST_FRAME_ON_EXIT";
const REPORT_TIMEOUT: Duration = Duration::from_secs(30);
const STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// The default executable: the bundled Niri when installed, otherwise `PATH`.
pub fn executable(compositor: Compositor) -> PathBuf {
    match compositor {
        Compositor::Niri if Path::new(BUNDLED_NIRI).is_file() => BUNDLED_NIRI.into(),
        Compositor::Niri => "niri".into(),
        Compositor::Cage => "cage".into(),
        Compositor::Weston => "weston".into(),
    }
}

/// The exec'd command for Cage or Weston, which run the greeter and exit with it.
pub fn command(
    compositor: Compositor,
    executable: Option<&Path>,
    renderer: Renderer,
    extra: &[OsString],
    greeter: &Path,
    arguments: &[OsString],
) -> Command {
    let program = executable.map_or_else(|| self::executable(compositor), Path::to_path_buf);
    let mut command = Command::new(program);
    match compositor {
        Compositor::Niri => unreachable!("Niri is supervised, see Niri::new"),
        Compositor::Cage => {
            command.args(["-s", "-m", "last"]);
            if matches!(renderer, Renderer::Software) {
                command.env("WLR_RENDERER", "pixman");
            }
        }
        Compositor::Weston => {
            command.args(["--shell=kiosk-shell.so", "--no-config", "--idle-time=0"]);
            if matches!(renderer, Renderer::Software) {
                command.arg("--renderer=pixman");
            }
        }
    }
    command.args(extra).arg("--").arg(greeter).args(arguments);
    command
}

/// The Niri login session: compositor command, greeter command and the private
/// runtime directory holding the generated configuration and report socket.
pub struct Niri {
    pub compositor: Command,
    pub greeter: Command,
    pub runtime: PathBuf,
    pub config: String,
}
impl Niri {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        executable: Option<&Path>,
        renderer: Renderer,
        extra: &[OsString],
        greeter: &Path,
        arguments: &[OsString],
        runtime_root: &Path,
        background: Rgb,
        displays: Option<&Path>,
    ) -> Self {
        let runtime = runtime_root.join(format!("canopa-greeter-{}", std::process::id()));
        let mut compositor = Command::new(
            executable.map_or_else(|| self::executable(Compositor::Niri), Path::to_path_buf),
        );
        compositor
            .arg("--config")
            .arg(runtime.join("niri.kdl"))
            .args(extra)
            .arg("--")
            .arg(greeter)
            .arg("internal-report-display")
            .arg("--socket")
            .arg(runtime.join("display.sock"))
            // A login compositor is not a systemd service and owns no session bus names.
            .env("NIRI_DISABLE_SYSTEM_MANAGER_NOTIFY", "1")
            .env(KEEP_LAST_FRAME, "1")
            .env_remove("NIRI_SOCKET")
            .env_remove("NIRI_CONFIG")
            .env_remove("NOTIFY_SOCKET");
        if matches!(renderer, Renderer::Software) {
            compositor.env("LIBGL_ALWAYS_SOFTWARE", "1");
        }
        let mut client = Command::new(greeter);
        client.args(arguments).env_remove("WAYLAND_SOCKET");
        Self {
            compositor,
            greeter: client,
            runtime,
            config: config(background, displays),
        }
    }

    /// Start Niri, then the greeter, and return when both have exited.
    pub fn run(mut self) -> Result<()> {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&self.runtime)
            .with_context(|| format!("could not create {}", self.runtime.display()))?;
        let _cleanup = Cleanup(self.runtime.clone());
        {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(self.runtime.join("niri.kdl"))?;
            file.write_all(self.config.as_bytes())?;
        }
        let report = UnixDatagram::bind(self.runtime.join("display.sock"))?;
        report.set_read_timeout(Some(Duration::from_millis(100)))?;
        let niri = spawn(&mut self.compositor).context("could not start Niri")?;
        let display = match wait_for_display(&report, niri) {
            Ok(display) => display,
            Err(error) => {
                stop(niri);
                return Err(error);
            }
        };
        drop(report);
        self.greeter.env("WAYLAND_DISPLAY", &display);
        let greeter = match spawn(&mut self.greeter) {
            Ok(pid) => pid,
            Err(error) => {
                stop(niri);
                return Err(error).context("could not start the login screen");
            }
        };
        let watched = match (pidfd(niri), pidfd(greeter)) {
            (Ok(niri), Ok(greeter)) => [niri, greeter],
            (Err(error), _) | (_, Err(error)) => {
                stop(greeter);
                stop(niri);
                return Err(error);
            }
        };
        loop {
            let (pid, status) = wait_either([niri, greeter], &watched)?;
            if pid == greeter {
                stop(niri);
                ensure!(
                    libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0,
                    "the login screen ended without starting a session ({})",
                    describe(status)
                );
                return Ok(());
            }
            if pid == niri {
                stop(greeter);
                bail!("Niri exited before the login screen ({})", describe(status));
            }
        }
    }
}

/// The compositor configuration for the login screen. Nothing in it starts
/// programs or binds keys: the only client is the greeter, fullscreen.
pub fn config(background: Rgb, displays: Option<&Path>) -> String {
    let Rgb(r, g, b) = background;
    let mut text = format!(
        r##"// Generated by canopa-greeter for the login screen. Changes are not kept.
input {{
    // logind handles the power button, as it does without a session.
    disable-power-key-handling
}}
prefer-no-csd
hotkey-overlay {{ skip-at-startup; }}
xwayland-satellite {{ off; }}
screenshot-path null
gestures {{
    hot-corners {{ off; }}
}}
layout {{
    gaps 0
    background-color "#{r:02x}{g:02x}{b:02x}"
    focus-ring {{ off; }}
    border {{ off; }}
    shadow {{ off; }}
}}
window-rule {{
    match app-id="^org\\.canopa\\.Greeter$"
    open-fullscreen true
}}
"##
    );
    if let Some(path) = displays {
        text.push_str(&format!("include {}\n", quote(&path.to_string_lossy())));
    }
    text
}

/// A KDL v1 string; `trusted_displays` admits no control characters.
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `displays` when it exists and, unless `explicit`, is root-owned along its
/// path. An untrusted file is skipped with a warning rather than blocking login.
pub fn trusted_displays(path: &Path, explicit: bool) -> Option<PathBuf> {
    if !path.exists() {
        return None;
    }
    if path
        .to_str()
        .is_none_or(|text| text.chars().any(char::is_control))
    {
        eprintln!("canopa-greeter: ignoring a display file path that is not plain text");
        return None;
    }
    if !explicit && let Err(error) = crate::config::check_system_source(path) {
        eprintln!(
            "canopa-greeter: ignoring {}: {error:#}; using the compositor's display defaults",
            path.display()
        );
        return None;
    }
    Some(path.to_path_buf())
}

/// Run inside Niri: send the compositor's `WAYLAND_DISPLAY` to the launcher.
pub fn report_display(socket: &Path) -> Result<()> {
    let display = std::env::var_os("WAYLAND_DISPLAY").context("WAYLAND_DISPLAY is not set")?;
    ensure!(valid_display(&display), "unexpected WAYLAND_DISPLAY");
    UnixDatagram::unbound()?.send_to(display.as_encoded_bytes(), socket)?;
    Ok(())
}

fn valid_display(display: &OsStr) -> bool {
    let bytes = display.as_encoded_bytes();
    !bytes.is_empty() && bytes.len() <= 100 && !bytes.iter().any(|b| matches!(b, b'/' | 0))
}

fn wait_for_display(report: &UnixDatagram, niri: libc::pid_t) -> Result<OsString> {
    let deadline = Instant::now() + REPORT_TIMEOUT;
    let mut buffer = [0; 128];
    loop {
        match report.recv(&mut buffer) {
            Ok(length) => {
                use std::os::unix::ffi::OsStringExt;
                let display = OsString::from_vec(buffer[..length].to_vec());
                ensure!(valid_display(&display), "Niri reported an invalid display");
                return Ok(display);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(error.into()),
        }
        if let Some(status) = try_wait(niri)? {
            bail!("Niri exited during startup ({})", describe(status));
        }
        ensure!(
            Instant::now() < deadline,
            "Niri did not start the login screen within {} s",
            REPORT_TIMEOUT.as_secs()
        );
    }
}

fn spawn(command: &mut Command) -> Result<libc::pid_t> {
    // SAFETY: prctl is async-signal-safe. Children stop when the launcher dies,
    // so greetd stopping the launcher never leaves a compositor holding the display.
    unsafe {
        command.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = command.spawn()?;
    // Reaped with waitpid below; std's handle is never waited on.
    Ok(child.id() as libc::pid_t)
}

fn pidfd(pid: libc::pid_t) -> Result<std::os::fd::OwnedFd> {
    use std::os::fd::FromRawFd;
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error()).context("pidfd_open");
    }
    // SAFETY: pidfd_open returned a new descriptor that nothing else owns.
    Ok(unsafe { std::os::fd::OwnedFd::from_raw_fd(fd as i32) })
}

/// Block until one of exactly these two children exits, then reap it. Never
/// `waitpid(-1)`: that would also reap children the process started elsewhere.
fn wait_either(
    pids: [libc::pid_t; 2],
    fds: &[std::os::fd::OwnedFd; 2],
) -> Result<(libc::pid_t, libc::c_int)> {
    use std::os::fd::AsRawFd;
    loop {
        let mut poll = fds.each_ref().map(|fd| libc::pollfd {
            fd: fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        });
        if unsafe { libc::poll(poll.as_mut_ptr(), 2, -1) } < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        for (entry, pid) in poll.iter().zip(pids) {
            if entry.revents != 0 {
                return wait(pid);
            }
        }
    }
}

fn wait(pid: libc::pid_t) -> Result<(libc::pid_t, libc::c_int)> {
    let mut status = 0;
    loop {
        let result = unsafe { libc::waitpid(pid, &mut status, 0) };
        if result >= 0 {
            return Ok((result, status));
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(error.into());
        }
    }
}

fn try_wait(pid: libc::pid_t) -> Result<Option<libc::c_int>> {
    let mut status = 0;
    match unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) } {
        0 => Ok(None),
        result if result == pid => Ok(Some(status)),
        _ => Err(std::io::Error::last_os_error().into()),
    }
}

/// SIGTERM, then SIGKILL after the timeout; always reaps the child.
fn stop(pid: libc::pid_t) {
    unsafe { libc::kill(pid, libc::SIGTERM) };
    let deadline = Instant::now() + STOP_TIMEOUT;
    while Instant::now() < deadline {
        match try_wait(pid) {
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            _ => return,
        }
    }
    eprintln!("canopa-greeter: process {pid} ignored SIGTERM; killing it");
    unsafe { libc::kill(pid, libc::SIGKILL) };
    let _ = wait(pid);
}

fn describe(status: libc::c_int) -> String {
    if libc::WIFEXITED(status) {
        format!("exit status {}", libc::WEXITSTATUS(status))
    } else if libc::WIFSIGNALED(status) {
        format!("signal {}", libc::WTERMSIG(status))
    } else {
        format!("wait status {status}")
    }
}

struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
