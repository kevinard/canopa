//! One bounded request per connection; the socket is scoped to the Wayland session.
use crate::{background::Fit, settings::Background};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::PathBuf,
    time::Duration,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Request {
    Set {
        image: PathBuf,
        output: Option<String>,
        fit: Fit,
        #[serde(default = "default_persist")]
        persist: bool,
    },
    SlideshowStart {
        directory: PathBuf,
        interval: u64,
        output: Option<String>,
        fit: Fit,
    },
    SlideshowStop {},
    Next {},
    Status {},
    /// Re-read the configuration and keep saved choices.
    Reload {},
    /// Re-read the configuration and forget every saved choice.
    Reset {},
    Stop {},
}
fn default_persist() -> bool {
    true
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Output {
    pub name: String,
    pub background: Background,
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_scale")]
    pub scale: u32,
    /// The output shows Canopa's embedded wallpaper because no image is assigned.
    #[serde(default)]
    pub built_in: bool,
}
fn default_scale() -> u32 {
    1
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub error: Option<String>,
    pub outputs: Vec<Output>,
    #[serde(default)]
    pub slideshow: Option<crate::wallpaper_state::Slideshow>,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
}
impl Response {
    pub fn ok() -> Self {
        Self {
            error: None,
            outputs: vec![],
            slideshow: None,
            color: None,
        }
    }
    pub fn error(error: impl std::fmt::Display) -> Self {
        Self {
            error: Some(error.to_string()),
            outputs: vec![],
            slideshow: None,
            color: None,
        }
    }
}
pub fn socket_path() -> Result<PathBuf> {
    scoped_socket_path("wallpaper")
}
pub(crate) fn scoped_socket_path(component: &str) -> Result<PathBuf> {
    ensure!(
        component
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'-'),
        "invalid endpoint component"
    );
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .context("XDG_RUNTIME_DIR is required")?;
    let metadata = std::fs::symlink_metadata(&runtime)?;
    // SAFETY: getuid has no preconditions.
    ensure!(
        runtime.is_absolute()
            && metadata.is_dir()
            && metadata.uid() == unsafe { libc::getuid() }
            && metadata.mode() & 0o077 == 0,
        "XDG_RUNTIME_DIR must be a private directory owned by the current user"
    );
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".into());
    let key = canopa_config::fingerprint(&display);
    Ok(runtime.join(format!("canopa-{component}-{}.sock", &key[..16])))
}
pub struct Server {
    pub listener: UnixListener,
    path: PathBuf,
    _lock: File,
}
impl Server {
    pub fn bind() -> Result<Self> {
        Self::bind_path(socket_path()?)
    }
    pub(crate) fn bind_path(path: PathBuf) -> Result<Self> {
        let lock = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path.with_extension("lock"))?;
        // The lock file stays in place: unlinking it would allow two flock identities.
        // SAFETY: lock is a live file descriptor and flock does not retain pointers.
        ensure!(
            unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "wallpaper daemon already running for this Wayland display"
        );
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        let listener = UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self {
            listener,
            path,
            _lock: lock,
        })
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
pub fn read_request(stream: &mut UnixStream) -> Result<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut bytes = Vec::new();
    stream.take(16385).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 16384, "request exceeds 16 KiB");
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn respond(stream: &mut UnixStream, response: &Response) -> Result<()> {
    stream.set_write_timeout(Some(Duration::from_millis(200)))?;
    stream.write_all(&serde_json::to_vec(response)?)?;
    Ok(())
}
pub fn send(request: &Request) -> Result<Response> {
    send_with_timeout(request, Duration::from_secs(60))
}
pub fn send_with_timeout(request: &Request, timeout: Duration) -> Result<Response> {
    let mut stream = UnixStream::connect(socket_path()?)
        .context("wallpaper daemon is unavailable; start canopa-wallpaper daemon")?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.write_all(&serde_json::to_vec(request)?)?;
    stream.shutdown(std::net::Shutdown::Write)?;
    let mut bytes = Vec::new();
    stream.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1024 * 1024, "response exceeds 1 MiB");
    let response: Response = serde_json::from_slice(&bytes)?;
    if let Some(error) = &response.error {
        anyhow::bail!("{error}");
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn server_lock_excludes_duplicates_and_recovers_stale_sockets() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("wallpaper.sock");
        let first = Server::bind_path(path.clone()).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        assert!(Server::bind_path(path.clone()).is_err());
        drop(first);
        assert!(!path.exists());
        drop(UnixListener::bind(&path).unwrap());
        let second = Server::bind_path(path.clone()).unwrap();
        assert!(path.exists());
        drop(second);
    }
    #[test]
    fn unknown_fields_and_oversized_requests_are_rejected() {
        for bytes in [
            br#"{"command":"status","extra":true}"#.to_vec(),
            vec![b' '; 16385],
        ] {
            let (mut reader, mut writer) = UnixStream::pair().unwrap();
            std::thread::spawn(move || {
                writer.write_all(&bytes).unwrap();
                writer.shutdown(std::net::Shutdown::Write).unwrap();
            });
            assert!(read_request(&mut reader).is_err());
        }
    }
}
