//! Anonymous-pipe dialog transport. The backend owns helper executable selection.
#[cfg(feature = "desktop")]
pub(crate) mod fd;
use std::{
    ffi::OsString,
    os::fd::{AsRawFd, BorrowedFd},
    path::PathBuf,
    process::Stdio,
};

pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct DialogProcess {
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
}

impl DialogProcess {
    pub(crate) fn spawn(
        &self,
        helper: &str,
        activation_token: Option<&str>,
    ) -> std::io::Result<tokio::process::Child> {
        self.spawn_inheriting(helper, activation_token, &[])
    }
    pub(crate) fn spawn_inheriting(
        &self,
        helper: &str,
        activation_token: Option<&str>,
        descriptors: &[BorrowedFd<'_>],
    ) -> std::io::Result<tokio::process::Child> {
        if descriptors.len() > 16 || descriptors.iter().any(|fd| fd.as_raw_fd() < 3) {
            return Err(std::io::Error::other("Invalid dialog descriptor set"));
        }
        let mut command = tokio::process::Command::new(&self.executable);
        command
            .args(&self.arguments)
            .arg(helper)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .env_remove("XDG_ACTIVATION_TOKEN")
            .env_remove("DESKTOP_STARTUP_ID")
            .kill_on_drop(true);
        if let Some(token) = activation_token {
            command.env("XDG_ACTIVATION_TOKEN", token);
        }
        #[cfg(target_os = "linux")]
        {
            let parent = rustix::process::getpid();
            let inherited: Vec<_> = descriptors.iter().map(AsRawFd::as_raw_fd).collect();
            // SAFETY: this runs only direct syscalls between fork and exec.
            // The closure captures a numeric PID/FD list and performs no allocation,
            // locking or access to other process state. Runtime worker threads
            // that spawn helpers live until backend runtime shutdown.
            unsafe {
                command.pre_exec(move || {
                    rustix::process::set_parent_process_death_signal(Some(
                        rustix::process::Signal::KILL,
                    ))?;
                    if rustix::process::getppid() != Some(parent) {
                        return Err(rustix::io::Errno::CHILD.into());
                    }
                    for descriptor in &inherited {
                        // Borrowed input files remain alive until spawn returns.
                        let fd = BorrowedFd::borrow_raw(*descriptor);
                        let flags = rustix::io::fcntl_getfd(fd)?;
                        rustix::io::fcntl_setfd(fd, flags & !rustix::io::FdFlags::CLOEXEC)?;
                    }
                    Ok(())
                });
            }
        }
        command.spawn()
    }
    pub(crate) async fn exchange<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        helper: &str,
        request: T,
        cancelled: tokio::sync::watch::Receiver<bool>,
    ) -> anyhow::Result<Option<R>> {
        self.exchange_activated(helper, request, cancelled, None)
            .await
    }
    pub(crate) async fn exchange_activated<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        helper: &str,
        request: T,
        cancelled: tokio::sync::watch::Receiver<bool>,
        activation_token: Option<&str>,
    ) -> anyhow::Result<Option<R>> {
        self.exchange_inheriting(helper, request, cancelled, activation_token, &[])
            .await
    }
    pub(crate) async fn exchange_inheriting<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        helper: &str,
        request: T,
        mut cancelled: tokio::sync::watch::Receiver<bool>,
        activation_token: Option<&str>,
        descriptors: &[BorrowedFd<'_>],
    ) -> anyhow::Result<Option<R>> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        if *cancelled.borrow() {
            return Ok(None);
        }
        let data = serde_json::to_vec(&request)?;
        anyhow::ensure!(data.len() <= MAX_MESSAGE_BYTES, "Dialog request too large");
        let mut child = self.spawn_inheriting(helper, activation_token, descriptors)?;
        let mut input = child.stdin.take().expect("piped stdin");
        let output = child.stdout.take().expect("piped stdout");
        let interaction = async {
            input.write_all(&data).await?;
            input.shutdown().await?;
            drop(input);
            let mut bytes = Vec::new();
            output
                .take(MAX_MESSAGE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .await?;
            anyhow::ensure!(
                bytes.len() <= MAX_MESSAGE_BYTES,
                "Dialog response too large"
            );
            Ok::<_, anyhow::Error>(bytes)
        };
        let bytes = tokio::select! {
            biased;
            _ = async { if !*cancelled.borrow() { let _ = cancelled.changed().await; } } => {
                child.kill().await?;
                return Ok(None);
            }
            result = interaction => result?,
        };
        anyhow::ensure!(child.wait().await?.success(), "Dialog stopped");
        if *cancelled.borrow() {
            return Ok(None);
        }
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[cfg(feature = "desktop")]
pub fn read_frame<T: serde::de::DeserializeOwned>(
    reader: &mut impl std::io::BufRead,
) -> anyhow::Result<T> {
    use std::io::BufRead;
    let mut bytes = Vec::new();
    std::io::Read::take(&mut *reader, MAX_MESSAGE_BYTES as u64 + 1)
        .read_until(b'\n', &mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= MAX_MESSAGE_BYTES && bytes.last() == Some(&b'\n'),
        "Invalid dialog control frame"
    );
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(all(test, feature = "desktop"))]
mod tests {
    use super::*;
    #[test]
    fn bounded_frames_keep_following_updates_and_reject_incomplete_messages() {
        let mut input = std::io::Cursor::new(b"[\"one.desktop\"]\n[\"two.desktop\"]\n");
        assert_eq!(
            read_frame::<Vec<String>>(&mut input).unwrap(),
            ["one.desktop"]
        );
        assert_eq!(
            read_frame::<Vec<String>>(&mut input).unwrap(),
            ["two.desktop"]
        );
        assert!(read_frame::<Vec<String>>(&mut input).is_err());
        assert!(read_frame::<Vec<String>>(&mut std::io::Cursor::new(b"[\"incomplete\"]")).is_err());
        let mut oversized = vec![b' '; MAX_MESSAGE_BYTES];
        oversized.push(b'\n');
        assert!(read_frame::<serde_json::Value>(&mut std::io::Cursor::new(oversized)).is_err());
    }
}
