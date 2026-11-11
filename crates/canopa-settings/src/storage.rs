use anyhow::{Context, Result, ensure};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

pub const MAX_BYTES: usize = canopa_config::MAX_BYTES;

/// The editor's exact source, including namespace comments and filesystem identity.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub path: PathBuf,
    pub text: String,
    identity: Option<(u64, u64)>,
}

impl Snapshot {
    pub fn load(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(value) => Some(value),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let (text, identity) = if let Some(metadata) = metadata {
            ensure!(
                metadata.is_file() && metadata.uid() == unsafe { libc::geteuid() },
                "{}",
                fl!("settings-error-not-owned")
            );
            let text = String::from_utf8(canopa_file::read_canonical_bounded(&path, MAX_BYTES)?)?;
            let after = std::fs::symlink_metadata(&path)?;
            ensure!(
                after.is_file() && after.dev() == metadata.dev() && after.ino() == metadata.ino(),
                "settings source changed while reading"
            );
            (text, Some((metadata.dev(), metadata.ino())))
        } else {
            (String::new(), None)
        };
        Ok(Self {
            path,
            text,
            identity,
        })
    }

    /// Whether the file existed when this snapshot was taken.
    pub fn exists(&self) -> bool {
        self.identity.is_some()
    }

    /// Whether both snapshots observed the same version of the same file.
    pub fn same(&self, other: &Self) -> bool {
        self.path == other.path && self.identity == other.identity && self.text == other.text
    }

    pub fn unchanged(&self) -> Result<()> {
        let now = Self::load(&self.path)?;
        ensure!(
            now.identity == self.identity && now.text == self.text,
            "{}",
            fl!("settings-error-changed-outside")
        );
        Ok(())
    }

    /// Validate the exact candidate before publishing it. The lock serializes
    /// cooperating editors; the final snapshot check detects external writers.
    pub fn save(
        &self,
        candidate: &str,
        validate: impl FnOnce(&Path) -> Result<()>,
    ) -> Result<Self> {
        ensure!(
            candidate.len() <= MAX_BYTES,
            "{}",
            fl!("settings-error-too-large")
        );
        let parent = self
            .path
            .parent()
            .context("settings need a parent directory")?;
        std::fs::create_dir_all(parent)?;
        let _lock = lock(&parent.join(".canopa-settings.lock"))?;
        self.unchanged()?;
        let mut temporary = tempfile::Builder::new()
            .prefix(".canopa-settings-")
            .tempfile_in(parent)?;
        temporary
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))?;
        temporary.write_all(candidate.as_bytes())?;
        temporary.as_file().sync_all()?;
        validate(temporary.path())?;
        self.unchanged()?;
        temporary.persist(&self.path)?;
        File::open(parent)?.sync_all()?;
        Self::load(&self.path)
    }
}

pub fn lock(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o077 == 0,
        "invalid settings lock file"
    );
    // A child process forked by another thread holds a copy of every open
    // descriptor until it executes, including a lock descriptor this process
    // has just closed. Retry for a short, bounded time so that window is not
    // mistaken for a concurrent settings operation.
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    loop {
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(file);
        }
        ensure!(
            std::time::Instant::now() < deadline,
            "{}",
            fl!("settings-error-busy")
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

pub fn quote(value: &str) -> String {
    let mut entry = kdl::KdlEntry::new(value.to_owned());
    entry.ensure_v1();
    entry.to_string().trim().to_owned()
}

/// Commands never pass through a shell, have a deadline and bounded captured output.
pub fn command(program: impl AsRef<std::ffi::OsStr>, args: &[String]) -> Result<String> {
    use std::{
        io::Read,
        os::unix::process::CommandExt,
        process::{Command, Stdio},
        sync::mpsc,
        time::{Duration, Instant},
    };
    let mut process = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    struct Cleanup(std::process::Child);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            // Descendants must not retain a captured pipe beyond the deadline.
            unsafe { libc::kill(-(self.0.id() as i32), libc::SIGKILL) };
            let _ = self.0.wait();
        }
    }
    let stdout = process.stdout.take().context("missing command output")?;
    let stderr = process
        .stderr
        .take()
        .context("missing command diagnostics")?;
    let mut process = Cleanup(process);
    let (out_tx, out_rx) = mpsc::channel();
    let (err_tx, err_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = out_tx.send(result);
    });
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stderr
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = err_tx.send(result);
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut out = None;
    let mut err = None;
    let (status, out, err) = loop {
        if out.is_none() {
            out = out_rx.try_recv().ok().transpose()?;
        }
        if err.is_none() {
            err = err_rx.try_recv().ok().transpose()?;
        }
        ensure!(
            out.as_ref().is_none_or(|v| v.len() <= MAX_BYTES)
                && err.as_ref().is_none_or(|v| v.len() <= MAX_BYTES),
            "command output exceeds budget"
        );
        if let Some(status) = process.0.try_wait()?
            && out.is_some()
            && err.is_some()
        {
            break (status, out.unwrap(), err.unwrap());
        }
        if Instant::now() >= deadline {
            anyhow::bail!("settings command timed out");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    ensure!(
        status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&err),
        String::from_utf8_lossy(&out[..out.len().min(8192)])
    );
    Ok(String::from_utf8(out)?)
}

pub fn companion(name: &str) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join(name)))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from(name))
}

/// Wait for an independently opened application without imposing a validator's
/// deadline on its window lifetime. Call from a detached worker, never the UI.
pub fn application(program: impl AsRef<std::ffi::OsStr>, args: &[String]) -> Result<()> {
    use std::{
        io::Read,
        os::unix::process::CommandExt,
        process::{Command, Stdio},
    };
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let mut diagnostic = Vec::new();
    let read = child
        .stderr
        .take()
        .context("missing application diagnostics")?
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut diagnostic);
    if read.is_err() || diagnostic.len() > MAX_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        read?;
        anyhow::bail!("application diagnostics exceed budget");
    }
    let status = child.wait()?;
    ensure!(
        status.success(),
        "{}",
        String::from_utf8_lossy(&diagnostic[..diagnostic.len().min(8192)])
    );
    Ok(())
}
