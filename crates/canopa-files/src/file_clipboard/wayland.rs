//! File clipboard transport on Canopa's Wayland compositor. Every protocol and
//! pipe operation runs on one worker, independently of Iced's text clipboard.
use super::Selection;
use futures_channel::{mpsc as asynchronous, oneshot};
use std::{
    io::{self, Write},
    os::unix::net::UnixStream,
    sync::{Arc, Mutex, mpsc},
};
mod pipes;
mod worker;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Intent {
    nonce: [u8; 16],
    sequence: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub ready: bool,
    pub revision: u64,
    pub owned: Option<Intent>,
    pub has_files: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Received {
    pub selection: Selection,
    pub owned: Option<Intent>,
}

type Reply<T> = oneshot::Sender<Result<T, String>>;
enum Control {
    Publish(Selection, Reply<Intent>),
    Read(Reply<Received>),
    ReplaceCut(Intent, Option<Selection>, Reply<bool>),
    Stop,
}
impl Control {
    fn reject(self, reason: &str) {
        match self {
            Self::Publish(_, reply) => {
                let _ = reply.send(Err(reason.into()));
            }
            Self::Read(reply) => {
                let _ = reply.send(Err(reason.into()));
            }
            Self::ReplaceCut(_, _, reply) => {
                let _ = reply.send(Err(reason.into()));
            }
            Self::Stop => {}
        }
    }
}

struct Shared {
    snapshot: Mutex<Snapshot>,
    notice: Mutex<asynchronous::Sender<()>>,
}
impl Shared {
    fn set(&self, snapshot: Snapshot) {
        *self.snapshot.lock().unwrap_or_else(|e| e.into_inner()) = snapshot;
        let _ = self
            .notice
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_send(());
    }
}

struct Handle {
    sender: mpsc::SyncSender<Control>,
    wake: UnixStream,
    shared: Arc<Shared>,
    notices: Mutex<Option<asynchronous::Receiver<()>>>,
}
impl Handle {
    fn wake(&self) {
        let _ = (&self.wake).write(&[1]);
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.sender.try_send(Control::Stop);
        self.wake();
    }
}

#[derive(Clone)]
pub struct Clipboard(Arc<Handle>);
impl Clipboard {
    /// Starts without performing display or filesystem I/O on the caller.
    pub fn start() -> io::Result<Self> {
        let (wake, reader) = UnixStream::pair()?;
        wake.set_nonblocking(true)?;
        reader.set_nonblocking(true)?;
        let (sender, receiver) = mpsc::sync_channel(8);
        let (notice, notices) = asynchronous::channel(0);
        let shared = Arc::new(Shared {
            snapshot: Mutex::new(Snapshot::default()),
            notice: Mutex::new(notice),
        });
        let state = shared.clone();
        std::thread::Builder::new()
            .name("files-clipboard".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    worker::run(&receiver, reader, state.clone())
                }))
                .unwrap_or_else(|_| Err(io::Error::other(fl!("files-clipboard-worker-stopped"))));
                let mut snapshot = state
                    .snapshot
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                snapshot.ready = false;
                snapshot.owned = None;
                snapshot.has_files = false;
                snapshot.error = result.err().map(|error| error.to_string());
                let reason = snapshot
                    .error
                    .clone()
                    .unwrap_or_else(|| "The file clipboard stopped".into());
                state.set(snapshot);
                for control in receiver.try_iter() {
                    control.reject(&reason);
                }
            })?;
        Ok(Self(Arc::new(Handle {
            sender,
            wake,
            shared,
            notices: Mutex::new(Some(notices)),
        })))
    }

    fn request<T>(
        &self,
        control: impl FnOnce(Reply<T>) -> Control,
    ) -> io::Result<oneshot::Receiver<Result<T, String>>> {
        let (send, receive) = oneshot::channel();
        self.0
            .sender
            .try_send(control(send))
            .map_err(|_| io::Error::other(fl!("files-clipboard-busy-unavailable")))?;
        self.0.wake();
        Ok(receive)
    }
    pub fn publish(
        &self,
        selection: Selection,
    ) -> io::Result<oneshot::Receiver<Result<Intent, String>>> {
        self.request(|reply| Control::Publish(selection, reply))
    }
    pub fn read(&self) -> io::Result<oneshot::Receiver<Result<Received, String>>> {
        self.request(Control::Read)
    }
    /// Updates data served by the original source, or destroys that source when
    /// empty. It never sets a new selection, so a newer owner cannot be replaced.
    /// Remainders intersect the currently served files: late partial results
    /// cannot reintroduce files already removed by another confirmed result.
    pub fn replace_cut(
        &self,
        intent: Intent,
        remaining: Option<Selection>,
    ) -> io::Result<oneshot::Receiver<Result<bool, String>>> {
        self.request(|reply| Control::ReplaceCut(intent, remaining, reply))
    }
    pub fn snapshot(&self) -> Snapshot {
        self.0
            .shared
            .snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn take_notices(&self) -> Option<asynchronous::Receiver<()>> {
        self.0
            .notices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }
}
