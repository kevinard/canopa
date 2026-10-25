//! Bounded subprocess output and cancellation of the owned process group.
use crate::Cancellation;
use rustix::process::{Pid, Signal, kill_process_group};
use std::{
    io::{self, Read},
    os::{fd::AsFd, unix::process::CommandExt},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const OUTPUT_LIMIT: usize = 64 * 1024;
type CapturedOutput = io::Result<(Vec<u8>, bool)>;
type OutputReader = thread::JoinHandle<CapturedOutput>;

#[derive(Clone, Debug)]
pub struct CommandOutcome {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub cancelled: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = kill_process_group(Pid::from_child(&self.0), Signal::KILL);
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

struct Completion(Arc<AtomicBool>);
impl Drop for Completion {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

pub(crate) fn run(mut command: Command, cancellation: &Cancellation) -> io::Result<CommandOutcome> {
    if cancellation.is_cancelled() {
        return Ok(CommandOutcome {
            success: false,
            exit_code: None,
            cancelled: true,
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        });
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut child = ChildGuard(command.spawn()?);
    let completion = Completion(Arc::new(AtomicBool::new(false)));
    let stdout = capture(
        child.0.stdout.take().expect("piped stdout"),
        completion.0.clone(),
    )?;
    let stderr = capture(
        child.0.stderr.take().expect("piped stderr"),
        completion.0.clone(),
    )?;
    let mut cancelled = false;
    let status = loop {
        if cancellation.is_cancelled() {
            cancelled = true;
            let group = Pid::from_child(&child.0);
            let _ = kill_process_group(group, Signal::TERM);
            // Do not reap the leader before the final group signal: its PID then
            // remains reserved even if it exits while descendants ignore TERM.
            thread::sleep(Duration::from_millis(250));
            let _ = kill_process_group(group, Signal::KILL);
            break child.0.wait()?;
        } else if let Some(status) = child.0.try_wait()? {
            break status;
        } else {
            thread::sleep(Duration::from_millis(10));
            continue;
        }
    };
    completion.0.store(true, Ordering::Release);
    let (stdout, stdout_truncated) = stdout
        .join()
        .map_err(|_| io::Error::other("stdout reader failed"))??;
    let (stderr, stderr_truncated) = stderr
        .join()
        .map_err(|_| io::Error::other("stderr reader failed"))??;
    Ok(CommandOutcome {
        success: status.success() && !cancelled,
        exit_code: status.code(),
        cancelled,
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
    })
}

fn capture(
    mut reader: impl Read + AsFd + Send + 'static,
    complete: Arc<AtomicBool>,
) -> io::Result<OutputReader> {
    let flags = rustix::fs::fcntl_getfl(&reader)?;
    rustix::fs::fcntl_setfl(&reader, flags | rustix::fs::OFlags::NONBLOCK)?;
    Ok(thread::spawn(move || {
        let mut output = Vec::new();
        let mut buffer = [0; 8192];
        let mut truncated = false;
        let mut deadline = None;
        loop {
            if complete.load(Ordering::Acquire) {
                let until =
                    *deadline.get_or_insert_with(|| Instant::now() + Duration::from_millis(100));
                if Instant::now() >= until {
                    truncated = true;
                    break;
                }
            }
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    let keep = count.min(OUTPUT_LIMIT.saturating_sub(output.len()));
                    output.extend_from_slice(&buffer[..keep]);
                    truncated |= keep < count;
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if complete.load(Ordering::Acquire) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => return Err(error),
            }
        }
        Ok((output, truncated))
    }))
}
