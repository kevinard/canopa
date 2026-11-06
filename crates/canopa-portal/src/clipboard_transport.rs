//! Private clipboard transport for a consented session. This module does not
//! authorize access or expose a portal interface: the owning session must do so.
//! One worker owns the privileged connection and every revocable transfer pipe.
use crate::clipboard::{MAX_MIME_BYTES, MAX_TYPES, OWNER_PREFIX, validate_mime, validate_types};
use std::{
    io::{self, Write},
    os::{fd::OwnedFd, unix::net::UnixStream},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc,
    },
    time::Duration,
};
use tokio::sync::{mpsc as asynchronous, oneshot, watch};

mod pipes;
#[cfg(test)]
mod tests;
mod worker;

const MAX_TRANSFERS: usize = 16;
const IDLE: Duration = Duration::from_secs(30);
const LIFETIME: Duration = Duration::from_secs(300);
const STARTUP: Duration = Duration::from_secs(5);

/// Backend-wide serials cannot alias across sessions or replacement workers.
#[derive(Clone)]
pub(crate) struct Serials(Arc<AtomicU32>);
impl Default for Serials {
    fn default() -> Self {
        static SEQUENCE: std::sync::OnceLock<Arc<AtomicU32>> = std::sync::OnceLock::new();
        Self(SEQUENCE.get_or_init(|| Arc::new(AtomicU32::new(0))).clone())
    }
}
impl Serials {
    fn next(&self) -> Option<u32> {
        self.0
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .ok()
            .map(|value| value + 1)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub ready: bool,
    pub generation: u64,
    pub mime_types: Vec<String>,
    pub session_is_owner: bool,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct Transfer {
    pub generation: u64,
    pub serial: u32,
    pub mime_type: String,
}
type Reply<T> = oneshot::Sender<Result<T, String>>;
enum Command {
    Publish(Vec<String>, Reply<()>),
    Read(u64, String, Reply<OwnedFd>),
    Write(u32, Reply<OwnedFd>),
    Done(u32, bool, Reply<()>),
}

/// Independent revocation lets the grant supervisor close transferred pipes even
/// while the session actor is waiting on unrelated I/O.
pub(crate) struct Revoker {
    closing: Arc<AtomicBool>,
    wake: UnixStream,
}
impl Revoker {
    pub fn revoke(&self) {
        self.closing.store(true, Ordering::Release);
        let _ = (&self.wake).write(&[1]);
    }
}
/// Non-clonable ownership of one consented session's clipboard connection.
pub struct Bridge {
    sender: mpsc::SyncSender<Command>,
    wake: UnixStream,
    closing: Arc<AtomicBool>,
    snapshots: watch::Receiver<Snapshot>,
    transfers: Option<asynchronous::Receiver<Transfer>>,
    finished: Option<oneshot::Receiver<Result<(), String>>>,
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.closing.store(true, Ordering::Release);
        let _ = (&self.wake).write(&[1]);
    }
}
impl Bridge {
    /// Open a separate named display connection, pinned to the compositor PID
    /// already verified by the owning portal. Never consumes WAYLAND_SOCKET.
    pub async fn start(compositor_pid: u32) -> anyhow::Result<Self> {
        Self::start_with_serials(compositor_pid, Serials::default()).await
    }
    pub(crate) async fn start_with_serials(
        compositor_pid: u32,
        serials: Serials,
    ) -> anyhow::Result<Self> {
        let (_cancel, cancelled) = watch::channel(false);
        Self::start_scoped(compositor_pid, serials, cancelled).await
    }
    pub(crate) async fn start_scoped(
        compositor_pid: u32,
        serials: Serials,
        mut cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(!*cancelled.borrow(), "Clipboard startup cancelled");
        anyhow::ensure!(compositor_pid != 0, "Invalid clipboard compositor PID");
        let (wake, reader) = UnixStream::pair()?;
        wake.set_nonblocking(true)?;
        reader.set_nonblocking(true)?;
        let (sender, receiver) = mpsc::sync_channel(16);
        let (snapshots, changes) = watch::channel(Snapshot::default());
        let (events, transfers) = asynchronous::channel(MAX_TRANSFERS);
        let (finished, completion) = oneshot::channel();
        let closing = Arc::new(AtomicBool::new(false));
        let stopped = closing.clone();
        std::thread::Builder::new()
            .name("portal-clipboard".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    worker::run(
                        compositor_pid,
                        receiver,
                        reader,
                        snapshots.clone(),
                        events,
                        stopped,
                        serials,
                    )
                }))
                .unwrap_or_else(|_| Err(io::Error::other("Clipboard worker panicked")))
                .map_err(|error| error.to_string());
                snapshots.send_modify(|snapshot| {
                    snapshot.ready = false;
                    snapshot.mime_types.clear();
                    snapshot.session_is_owner = false;
                    snapshot.error = Some(
                        result
                            .as_ref()
                            .err()
                            .cloned()
                            .unwrap_or_else(|| "Clipboard stopped".into()),
                    );
                });
                let _ = finished.send(result);
            })?;
        let mut bridge = Self {
            sender,
            wake,
            closing,
            snapshots: changes,
            transfers: Some(transfers),
            finished: Some(completion),
        };
        let opening = tokio::time::timeout(STARTUP + Duration::from_secs(1), async {
            loop {
                let current = bridge.snapshots.borrow_and_update().clone();
                anyhow::ensure!(
                    current.error.is_none(),
                    "{}",
                    current.error.unwrap_or_default()
                );
                if current.ready {
                    break;
                }
                bridge.snapshots.changed().await?;
            }
            Ok::<_, anyhow::Error>(())
        });
        let result = tokio::select! {biased;
            _=async {while !*cancelled.borrow() {if cancelled.changed().await.is_err() {break;}}}=>Err(anyhow::anyhow!("Clipboard startup cancelled")),
            result=opening=>result.map_err(anyhow::Error::from).and_then(|value|value),
        };
        if let Err(error) = result {
            bridge.stop().await?;
            return Err(error);
        }
        Ok(bridge)
    }
    pub(crate) fn revoker(&self) -> io::Result<Revoker> {
        Ok(Revoker {
            closing: self.closing.clone(),
            wake: self.wake.try_clone()?,
        })
    }
    pub fn snapshots(&self) -> watch::Receiver<Snapshot> {
        self.snapshots.clone()
    }
    pub fn take_transfers(&mut self) -> Option<asynchronous::Receiver<Transfer>> {
        self.transfers.take()
    }
    fn queue(&self, command: Command) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.closing.load(Ordering::Acquire),
            "Clipboard is stopped"
        );
        self.sender
            .try_send(command)
            .map_err(|_| anyhow::anyhow!("Clipboard is busy or stopped"))?;
        let _ = (&self.wake).write(&[1]);
        Ok(())
    }
    pub async fn publish(&self, mime_types: Vec<String>) -> anyhow::Result<()> {
        validate_types(&mime_types)?;
        let (reply, result) = oneshot::channel();
        self.queue(Command::Publish(mime_types, reply))?;
        result.await?.map_err(anyhow::Error::msg)
    }
    /// The caller must pass the generation it authorized; replacement is an error.
    pub async fn read(&self, generation: u64, mime_type: String) -> anyhow::Result<OwnedFd> {
        validate_mime(&mime_type)?;
        let (reply, result) = oneshot::channel();
        self.queue(Command::Read(generation, mime_type, reply))?;
        result.await?.map_err(anyhow::Error::msg)
    }
    pub async fn write(&self, serial: u32) -> anyhow::Result<OwnedFd> {
        let (reply, result) = oneshot::channel();
        self.queue(Command::Write(serial, reply))?;
        result.await?.map_err(anyhow::Error::msg)
    }
    pub async fn write_done(&self, serial: u32, success: bool) -> anyhow::Result<()> {
        let (reply, result) = oneshot::channel();
        self.queue(Command::Done(serial, success, reply))?;
        result.await?.map_err(anyhow::Error::msg)
    }
    pub async fn stop(mut self) -> anyhow::Result<()> {
        self.closing.store(true, Ordering::Release);
        let _ = (&self.wake).write(&[1]);
        tokio::time::timeout(
            Duration::from_secs(2),
            self.finished.take().expect("clipboard completion"),
        )
        .await??
        .map_err(anyhow::Error::msg)
    }
}
