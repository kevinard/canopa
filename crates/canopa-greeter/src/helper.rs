//! Cancellable isolated helpers. Own the process group until it is killed and reaped.
use anyhow::{Context, Result, ensure};
use std::{
    io::Read,
    os::unix::{io::AsRawFd, process::CommandExt},
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;
const DEADLINE: Duration = Duration::from_secs(2);
struct Process {
    child: Child,
    active: bool,
}
impl Process {
    fn finish(&mut self) -> std::io::Result<ExitStatus> {
        // The leader has not been reaped, so its group ID cannot be reused here.
        unsafe { libc::kill(-(self.child.id() as i32), libc::SIGKILL) };
        let result = self.child.wait();
        self.active = false;
        result
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        if self.active {
            let _ = self.finish();
        }
    }
}

pub(crate) fn run(
    mut command: Command,
    cancelled: &dyn Fn() -> bool,
    maximum: usize,
) -> Result<Zeroizing<Vec<u8>>> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env_remove("GREETD_SOCK")
        .env_remove("NOTIFY_SOCKET")
        .current_dir("/")
        .process_group(0);
    let parent = unsafe { libc::getpid() };
    // SAFETY: only async-signal-safe scalar syscalls are used before exec.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(
                libc::PR_SET_PDEATHSIG,
                libc::SIGKILL as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
            ) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                return Err(std::io::Error::from_raw_os_error(libc::ECHILD));
            }
            Ok(())
        });
    }
    let mut process = Process {
        child: command.spawn().context("cannot start discovery helper")?,
        active: true,
    };
    let mut output = process
        .child
        .stdout
        .take()
        .context("missing discovery helper pipe")?;
    let fd = output.as_raw_fd();
    // SAFETY: fd is owned by the live output pipe; no other reader changes its flags.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        ensure!(
            flags >= 0 && libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) == 0,
            "cannot configure discovery output"
        );
    }
    let deadline = Instant::now() + DEADLINE;
    let mut bytes = Zeroizing::new(Vec::new());
    let mut eof = false;
    loop {
        ensure!(!cancelled(), "discovery cancelled");
        ensure!(Instant::now() < deadline, "discovery helper timed out");
        if !eof {
            loop {
                let mut chunk = [0u8; 4096];
                match output.read(&mut chunk) {
                    Ok(0) => {
                        eof = true;
                        break;
                    }
                    Ok(size) => {
                        ensure!(
                            bytes.len() + size <= maximum,
                            "discovery output exceeds budget"
                        );
                        bytes.extend_from_slice(&chunk[..size]);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e.into()),
                }
            }
        }
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // Observe exit without reaping; cleanup retains ownership of the process/group identity.
        let status = unsafe {
            libc::waitid(
                libc::P_PID,
                process.child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if status < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        ensure!(status == 0, "cannot observe discovery helper");
        if eof && unsafe { info.si_pid() } != 0 {
            break;
        }
        let mut poll = libc::pollfd {
            fd: if eof { -1 } else { fd },
            events: libc::POLLIN,
            revents: 0,
        };
        let delay = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(50) as i32;
        let status = unsafe { libc::poll(&mut poll, 1, delay) };
        if status < 0 {
            ensure!(
                std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted,
                "discovery helper poll failed"
            );
        }
    }
    ensure!(process.finish()?.success(), "discovery helper failed");
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_stops_and_reaps_a_blocked_helper_without_waiting_for_its_deadline() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("started");
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                "printf '%s' \"$$\" > \"$1\"; exec sleep 60",
                "fixture",
            ])
            .arg(&marker);
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let thread =
            std::thread::spawn(move || run(command, &|| flag.load(Ordering::Acquire), 65536));
        let started = Instant::now();
        while std::fs::read_to_string(&marker)
            .unwrap_or_default()
            .is_empty()
        {
            assert!(started.elapsed() < Duration::from_secs(1));
            std::thread::sleep(Duration::from_millis(5));
        }
        let pid = std::fs::read_to_string(marker).unwrap();
        let started = Instant::now();
        cancelled.store(true, Ordering::Release);
        assert!(
            thread
                .join()
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
        assert!(started.elapsed() < Duration::from_millis(500));
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    }
}
