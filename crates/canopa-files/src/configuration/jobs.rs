//! Ordered configuration work and publication, independent of any frontend.
use super::{ConfigurationRuntime, ConfigurationSnapshot, ReloadEvent};
use crate::{actions::CustomAction, interface::InterfaceSettings};
use futures_channel::{mpsc as asynchronous, oneshot};
use std::{
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

/// One complete publication, including the last valid candidate on rejection.
#[derive(Clone, Debug)]
pub struct Published {
    revision: u64,
    pub current_revision: u64,
    pub current: Arc<ConfigurationSnapshot>,
    pub error: Option<String>,
}

impl Published {
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub current: Arc<ConfigurationSnapshot>,
    pub current_revision: u64,
    pub latest: Option<Published>,
    pub pending: usize,
}

pub type Reply = Result<Published, String>;

enum Change {
    Save(ConfigurationSnapshot, Vec<CustomAction>),
    SaveSettings(ConfigurationSnapshot, InterfaceSettings),
    Reload(ConfigurationSnapshot),
}

impl Change {
    fn selection(&self) -> &canopa_config::Request {
        match self {
            Self::Save(base, _) | Self::SaveSettings(base, _) | Self::Reload(base) => &base.request,
        }
    }
}

struct ChangeRequest {
    change: Change,
    reply: oneshot::Sender<Reply>,
    pending: Pending,
}

enum Request {
    Change(Box<ChangeRequest>),
    Stop,
}

struct Shared {
    snapshot: Mutex<Snapshot>,
    notice: Mutex<asynchronous::Sender<()>>,
}

impl Shared {
    fn notify(&self) {
        let _ = self
            .notice
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_send(());
    }

    fn publish(
        &self,
        result: Result<ConfigurationSnapshot, String>,
        completed_request: bool,
    ) -> Reply {
        let mut snapshot = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        if completed_request {
            snapshot.pending -= 1;
        }
        let revision = snapshot.latest.as_ref().map_or(0, Published::revision) + 1;
        let error = match result {
            Ok(configuration) => {
                snapshot.current = Arc::new(configuration);
                snapshot.current_revision = revision;
                None
            }
            Err(error) => Some(error),
        };
        let publication = Published {
            revision,
            current_revision: snapshot.current_revision,
            current: snapshot.current.clone(),
            error: error.clone(),
        };
        snapshot.latest = Some(publication.clone());
        let reply = error.map_or_else(|| Ok(publication), Err);
        drop(snapshot);
        self.notify();
        reply
    }
}

/// Counts queued and running requests, including when a caller drops its reply.
/// Dropped/refused requests release their hold even during worker unwinding.
struct Pending {
    shared: Arc<Shared>,
    active: bool,
}

impl Pending {
    fn new(shared: Arc<Shared>) -> Self {
        shared
            .snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending += 1;
        Self {
            shared,
            active: true,
        }
    }

    fn complete(mut self, result: Result<ConfigurationSnapshot, String>) -> Reply {
        self.active = false;
        self.shared.publish(result, true)
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if self.active {
            self.shared
                .snapshot
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pending -= 1;
            self.shared.notify();
        }
    }
}

struct Handle {
    requests: mpsc::SyncSender<Request>,
    shared: Arc<Shared>,
    notices: Mutex<Option<asynchronous::Receiver<()>>>,
    stopped: Arc<AtomicBool>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.requests.try_send(Request::Stop);
    }
}

/// Clones share one owning runtime. Frontend detachment does not stop its work.
#[derive(Clone)]
pub struct ConfigurationJobs(Arc<Handle>);

impl ConfigurationJobs {
    pub fn start(snapshot: ConfigurationSnapshot) -> io::Result<Self> {
        let (requests, receiver) = mpsc::sync_channel(1);
        let (notice, notices) = asynchronous::channel(0);
        let shared = Arc::new(Shared {
            snapshot: Mutex::new(Snapshot {
                current: Arc::new(snapshot.clone()),
                current_revision: 0,
                latest: None,
                pending: 0,
            }),
            notice: Mutex::new(notice),
        });
        let worker = shared.clone();
        let stopped = Arc::new(AtomicBool::new(false));
        let stopping = stopped.clone();
        std::thread::Builder::new()
            .name("files-configuration".into())
            .spawn(move || {
                let selection = snapshot.request.clone();
                let mut runtime = ConfigurationRuntime::from_snapshot(snapshot);
                while !stopping.load(Ordering::Acquire) {
                    match receiver.recv_timeout(Duration::from_millis(250)) {
                        Ok(Request::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Ok(Request::Change(request)) => {
                            let ChangeRequest {
                                change,
                                reply,
                                pending,
                            } = *request;
                            let result = if change.selection() != &selection {
                                Err("This configuration belongs to another source selection."
                                    .into())
                            } else {
                                match change {
                                    Change::Save(base, actions) => base
                                        .save_actions(actions)
                                        .map_err(|error| error.to_string()),
                                    Change::SaveSettings(base, settings) => base
                                        .save_settings(settings)
                                        .map_err(|error| error.to_string()),
                                    Change::Reload(base) => {
                                        base.reload().map_err(|error| error.to_string())
                                    }
                                }
                            };
                            if let Ok(snapshot) = &result {
                                runtime = ConfigurationRuntime::from_snapshot(snapshot.clone());
                            }
                            let _ = reply.send(pending.complete(result));
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => match runtime.poll() {
                            ReloadEvent::Unchanged => {}
                            ReloadEvent::Updated(snapshot) => {
                                let _ = worker.publish(Ok(*snapshot), false);
                            }
                            ReloadEvent::Rejected(error) => {
                                let _ = worker.publish(Err(error.to_string()), false);
                            }
                        },
                    }
                }
                stopping.store(true, Ordering::Release);
            })?;
        Ok(Self(Arc::new(Handle {
            requests,
            shared,
            notices: Mutex::new(Some(notices)),
            stopped,
        })))
    }

    pub fn save(
        &self,
        base: ConfigurationSnapshot,
        actions: Vec<CustomAction>,
    ) -> io::Result<oneshot::Receiver<Reply>> {
        self.send(Change::Save(base, actions))
    }

    pub fn save_settings(
        &self,
        base: ConfigurationSnapshot,
        settings: InterfaceSettings,
    ) -> io::Result<oneshot::Receiver<Reply>> {
        self.send(Change::SaveSettings(base, settings))
    }

    pub fn reload(&self, base: ConfigurationSnapshot) -> io::Result<oneshot::Receiver<Reply>> {
        self.send(Change::Reload(base))
    }

    pub fn snapshot(&self) -> Snapshot {
        self.0
            .shared
            .snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// One coalesced wakeup stream; neither saves nor reloads need an observer.
    pub fn take_notices(&self) -> Option<asynchronous::Receiver<()>> {
        self.0
            .notices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    fn send(&self, change: Change) -> io::Result<oneshot::Receiver<Reply>> {
        if self.0.stopped.load(Ordering::Acquire) {
            return Err(io::Error::other("The configuration worker has stopped."));
        }
        let (reply, receiver) = oneshot::channel();
        let pending = Pending::new(self.0.shared.clone());
        self.0
            .requests
            .try_send(Request::Change(Box::new(ChangeRequest {
                change,
                reply,
                pending,
            })))
            .map_err(|_| io::Error::other("the configuration worker is busy or unavailable"))?;
        self.0.shared.notify();
        Ok(receiver)
    }
}
