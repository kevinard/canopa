//! Small, bounded notification status interface. No notification bodies, storage,
//! policy or UI dependencies cross this interface.
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub const SUMMARY_REQUEST: &str = "summary-subscribe-v1\n";
pub const MAX_SUMMARY_BYTES: usize = 512;
const CONNECT_TIMEOUT: Duration = Duration::from_millis(900);
const FRAME_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    /// Changes on every daemon start. This snapshot never authorizes item actions.
    pub instance: [u8; 16],
    pub requires_attention: u64,
    pub do_not_disturb: bool,
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
        let mut first = [0];
        if self.reader.read(&mut first).await? == 0 {
            return Ok(None);
        }
        anyhow::ensure!(first[0] != b'\n', "empty notification summary frame");
        let mut bytes = Vec::with_capacity(MAX_SUMMARY_BYTES + 1);
        bytes.push(first[0]);
        tokio::time::timeout(
            FRAME_TIMEOUT,
            (&mut self.reader)
                .take(MAX_SUMMARY_BYTES as u64)
                .read_until(b'\n', &mut bytes),
        )
        .await??;
        anyhow::ensure!(
            bytes.len() <= MAX_SUMMARY_BYTES && bytes.last() == Some(&b'\n'),
            "invalid notification summary frame"
        );
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| anyhow::anyhow!("invalid notification summary"))
    }
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
