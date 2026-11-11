//! Small, bounded notification status interface. No notification bodies, storage,
//! policy or UI dependencies cross this interface.
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub mod preferences;

pub const SUMMARY_REQUEST: &str = "summary-subscribe-v1\n";
pub const MAX_SUMMARY_BYTES: usize = 512;
/// Read-only lock-screen feed for the session locker. See [`LockScreenSnapshot`].
pub const LOCK_SCREEN_REQUEST: &str = "lock-screen-subscribe-v1\n";
pub const MAX_LOCK_SCREEN_BYTES: usize = 4096;
/// At most this many entries cross the lock-screen feed.
pub const MAX_LOCK_SCREEN_ENTRIES: usize = 4;
const CONNECT_TIMEOUT: Duration = Duration::from_millis(900);
const FRAME_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    /// Changes on every daemon start. This snapshot never authorizes item actions.
    pub instance: [u8; 16],
    pub requires_attention: u64,
    /// Effective Do Not Disturb, including timers and schedules.
    pub do_not_disturb: bool,
    /// Why Do Not Disturb is active; absent when it is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub do_not_disturb_reason: Option<DoNotDisturbReason>,
    /// Unix milliseconds when the current Do Not Disturb ends by itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub do_not_disturb_until: Option<i64>,
    /// Automatic banner suppression currently in effect, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppression: Option<Suppression>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DoNotDisturbReason {
    /// Turned on until turned off.
    Manual,
    /// Turned on for a duration or until a time.
    Timer,
    /// Inside the daily schedule.
    Schedule,
}

/// Automatic suppression. Screen sharing takes precedence over fullscreen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Suppression {
    /// An application is fullscreen on the focused output.
    Fullscreen,
    /// An output or window is being shared or recorded; content is hidden.
    ScreenSharing,
}

/// The lock-screen feed's complete current state. Notify applies the lock-screen
/// policy before anything crosses this interface: entries carry display text
/// only, never actions, targets, bodies or IDs, and hidden content never leaves
/// Notify. The locker renders it read-only.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockScreenSnapshot {
    pub entries: Vec<LockScreenEntry>,
    /// Matching notifications beyond the bounded entries.
    #[serde(default)]
    pub more: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockScreenEntry {
    /// Application display name.
    pub app: String,
    /// The summary when the app's rule allows content, otherwise a generic line.
    pub text: String,
    #[serde(default)]
    pub critical: bool,
}

impl LockScreenEntry {
    /// "App — text", bounded to one display line.
    pub fn line(&self) -> String {
        format!("{} — {}", self.app, self.text)
    }
}

/// The session socket used by the daemon, bar and built-in Center. Explicit
/// `--socket` remains available for standalone daemons and test fixtures.
pub fn default_socket() -> anyhow::Result<PathBuf> {
    socket_in(
        std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .as_deref(),
    )
}

fn socket_in(runtime: Option<&Path>) -> anyhow::Result<PathBuf> {
    let runtime = runtime.filter(|path| path.is_absolute())
        .ok_or_else(|| anyhow::anyhow!("XDG_RUNTIME_DIR must be an absolute path; use --socket for an explicit notification socket"))?;
    Ok(runtime.join("canopa-notify/control.sock"))
}

/// A subscription begins with a fresh complete snapshot. Slow readers may skip
/// intermediate states. EOF or any error invalidates the last snapshot; callers
/// reconnect and replace it, never merge across daemon lifetimes.
pub struct SummarySubscription {
    reader: BufReader<tokio::net::UnixStream>,
}

impl SummarySubscription {
    pub async fn connect(socket: &Path) -> anyhow::Result<(Self, Summary)> {
        tokio::time::timeout(CONNECT_TIMEOUT, async {
            let mut stream = tokio::net::UnixStream::connect(socket).await?;
            stream.write_all(SUMMARY_REQUEST.as_bytes()).await?;
            let mut subscription = Self {
                reader: BufReader::new(stream),
            };
            let snapshot = subscription
                .next()
                .await?
                .ok_or_else(|| anyhow::anyhow!("notification summary ended before its snapshot"))?;
            Ok((subscription, snapshot))
        })
        .await?
    }

    pub async fn next(&mut self) -> anyhow::Result<Option<Summary>> {
        // An idle stream has no deadline. Once a frame begins, a partial frame
        // cannot keep stale availability displayed indefinitely.
        let Some(bytes) = read_frame(&mut self.reader, MAX_SUMMARY_BYTES).await? else {
            return Ok(None);
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| anyhow::anyhow!("invalid notification summary"))
    }
}

/// Read-only lock-screen subscription with the same framing and reconnect
/// contract as [`SummarySubscription`].
pub struct LockScreenSubscription {
    reader: BufReader<tokio::net::UnixStream>,
}

impl LockScreenSubscription {
    pub async fn connect(socket: &Path) -> anyhow::Result<(Self, LockScreenSnapshot)> {
        tokio::time::timeout(CONNECT_TIMEOUT, async {
            let mut stream = tokio::net::UnixStream::connect(socket).await?;
            stream.write_all(LOCK_SCREEN_REQUEST.as_bytes()).await?;
            let mut subscription = Self {
                reader: BufReader::new(stream),
            };
            let snapshot = subscription.next().await?.ok_or_else(|| {
                anyhow::anyhow!("lock-screen notifications ended before their snapshot")
            })?;
            Ok((subscription, snapshot))
        })
        .await?
    }

    pub async fn next(&mut self) -> anyhow::Result<Option<LockScreenSnapshot>> {
        let Some(bytes) = read_frame(&mut self.reader, MAX_LOCK_SCREEN_BYTES).await? else {
            return Ok(None);
        };
        let snapshot: LockScreenSnapshot = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("invalid lock-screen notifications"))?;
        anyhow::ensure!(
            snapshot.entries.len() <= MAX_LOCK_SCREEN_ENTRIES,
            "too many lock-screen notifications"
        );
        Ok(Some(snapshot))
    }
}

async fn read_frame(
    reader: &mut BufReader<tokio::net::UnixStream>,
    maximum: usize,
) -> anyhow::Result<Option<Vec<u8>>> {
    let mut first = [0];
    if reader.read(&mut first).await? == 0 {
        return Ok(None);
    }
    anyhow::ensure!(first[0] != b'\n', "empty notification frame");
    let mut bytes = Vec::with_capacity(maximum + 1);
    bytes.push(first[0]);
    tokio::time::timeout(
        FRAME_TIMEOUT,
        reader.take(maximum as u64).read_until(b'\n', &mut bytes),
    )
    .await??;
    anyhow::ensure!(
        bytes.len() <= maximum && bytes.last() == Some(&b'\n'),
        "invalid notification frame"
    );
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_requires_an_absolute_runtime_directory() {
        assert!(socket_in(None).is_err());
        assert!(socket_in(Some(Path::new("relative"))).is_err());
        assert_eq!(
            socket_in(Some(Path::new("/run/user/123"))).unwrap(),
            Path::new("/run/user/123/canopa-notify/control.sock")
        );
    }

    #[tokio::test]
    async fn receives_snapshot_updates_and_disconnect_without_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("control.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let expected = Summary {
            instance: [9; 16],
            requires_attention: 42,
            do_not_disturb: true,
            do_not_disturb_reason: Some(DoNotDisturbReason::Timer),
            do_not_disturb_until: Some(1_700_000_000_000),
            suppression: Some(Suppression::ScreenSharing),
        };
        let peer = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut reader = BufReader::new(stream);
            let mut request = String::new();
            reader.read_line(&mut request).await.unwrap();
            assert_eq!(request, SUMMARY_REQUEST);
            for snapshot in [
                expected,
                Summary {
                    requires_attention: 0,
                    ..expected
                },
            ] {
                let line = serde_json::to_string(&snapshot).unwrap() + "\n";
                assert!(line.len() <= MAX_SUMMARY_BYTES);
                reader.get_mut().write_all(line.as_bytes()).await.unwrap();
            }
        });
        let (mut subscription, snapshot) = SummarySubscription::connect(&path).await.unwrap();
        assert_eq!(snapshot, expected);
        assert_eq!(
            subscription
                .next()
                .await
                .unwrap()
                .unwrap()
                .requires_attention,
            0
        );
        assert_eq!(subscription.next().await.unwrap(), None);
        peer.await.unwrap();
    }

    #[tokio::test]
    async fn rejects_malformed_incomplete_and_oversized_snapshots() {
        for frame in [
            b"{}\n".to_vec(),
            b"{}".to_vec(),
            vec![b'x'; MAX_SUMMARY_BYTES + 1],
            b"error unavailable\n".to_vec(),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("control.sock");
            let listener = tokio::net::UnixListener::bind(&path).unwrap();
            let peer = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut reader = BufReader::new(stream);
                let mut request = String::new();
                reader.read_line(&mut request).await.unwrap();
                reader.get_mut().write_all(&frame).await.unwrap();
            });
            assert!(SummarySubscription::connect(&path).await.is_err());
            peer.await.unwrap();
        }
    }
}
