//! Session-owned commands and completion attention, without a renderer.
use crate::{
    action_jobs::{ActionJobs, ActionRequest, Snapshot as QueueSnapshot},
    desktop_notifications::Worker,
};
use futures_channel::{mpsc, oneshot};
use futures_util::{StreamExt, stream::FuturesUnordered};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    sync::{Arc, Mutex},
};

// The notification transport has one running request and sixteen queued ones.
const DELIVERY_LIMIT: usize = 17;

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub queue: QueueSnapshot,
    pub pending_notifications: usize,
    pub notification_errors: BTreeMap<u64, String>,
    pub error: Option<String>,
}
impl Snapshot {
    pub fn holds_application(&self) -> bool {
        self.queue.holds_application() || self.pending_notifications > 0
    }
}

#[derive(Default)]
struct Attention {
    pending: BTreeSet<u64>,
    errors: BTreeMap<u64, String>,
    error: Option<String>,
}

#[derive(Clone)]
pub struct LocalActions(Arc<Handle>);
struct Handle {
    jobs: ActionJobs,
    attention: Arc<Mutex<Attention>>,
    notices: Mutex<Option<mpsc::Receiver<()>>>,
    stop: Mutex<Option<oneshot::Sender<()>>>,
    shutdown: tokio::sync::mpsc::Sender<oneshot::Sender<Result<(), String>>>,
}
impl Drop for Handle {
    fn drop(&mut self) {
        if let Some(stop) = self
            .stop
            .get_mut()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        {
            let _ = stop.send(());
        }
    }
}

impl LocalActions {
    /// Owns command scheduling and the bounded notification handoff. A transport
    /// startup error is reported per completion; it does not disable commands.
    pub fn start(notifications: Result<Worker, String>) -> io::Result<Self> {
        Self::start_with_activity(
            notifications,
            crate::local_activity::LocalActivity::default(),
        )
    }

    pub fn start_with_activity(
        notifications: Result<Worker, String>,
        activity: crate::local_activity::LocalActivity,
    ) -> io::Result<Self> {
        Self::start_jobs(notifications, ActionJobs::start_with_activity(activity)?)
    }

    pub fn start_with_storage(
        notifications: Result<Worker, String>,
        activity: crate::local_activity::LocalActivity,
        storage: crate::storage::PreparedStorage,
    ) -> io::Result<Self> {
        Self::start_jobs(
            notifications,
            ActionJobs::start_with_storage(activity, storage)?,
        )
    }

    fn start_jobs(notifications: Result<Worker, String>, jobs: ActionJobs) -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let mut changes = jobs
            .take_notices()
            .ok_or_else(|| io::Error::other("Action notices already claimed."))?;
        let worker_jobs = jobs.clone();
        let attention = Arc::new(Mutex::new(Attention::default()));
        let worker_attention = attention.clone();
        let (mut notice, notices) = mpsc::channel(0);
        let (stop, mut stopped) = oneshot::channel();
        let (shutdown, mut shutdown_requests) =
            tokio::sync::mpsc::channel::<oneshot::Sender<Result<(), String>>>(1);
        std::thread::Builder::new().name("files-action-attention".into()).spawn(move || {
            runtime.block_on(async move {
                let mut pending = FuturesUnordered::new();
                let mut open = true;
                loop {
                    tokio::select! {
                        _ = &mut stopped => return,
                        Some(reply) = shutdown_requests.recv() => {
                            let delivering = !worker_attention.lock().unwrap_or_else(|e| e.into_inner()).pending.is_empty();
                            let result = if delivering {
                                Err("Wait for custom-action notifications before stopping the runtime.".into())
                            } else {
                                // Keep unhanded receipts in the queue during its
                                // shutdown check, preserving their lifetime hold.
                                match worker_jobs.shutdown() {
                                    Ok(result) => result.await.unwrap_or_else(|_| Err("The custom-action runtime stopped before replying.".into())),
                                    Err(error) => Err(error.to_string()),
                                }
                            };
                            let _ = reply.send(result);
                        }
                        change = changes.next(), if open => {
                            if change.is_none() { open = false; }
                            // Readers take this same lock before inspecting the
                            // queue, so the completion hold has no handoff gap.
                            let completions = {
                                let mut state = worker_attention.lock().unwrap_or_else(|e| e.into_inner());
                                let completions = worker_jobs.take_completions();
                                state.pending.extend(completions.iter().map(|completion| completion.id));
                                completions
                            };
                            for completion in completions {
                                let id = completion.id;
                                let result = if pending.len() >= DELIVERY_LIMIT {
                                    Err("Desktop notifications are busy or unavailable.".into())
                                } else {
                                    notifications.as_ref().map_err(Clone::clone).and_then(|worker| {
                                        worker.send(completion.title, completion.status.description()).map_err(|error| error.to_string())
                                    })
                                };
                                match result {
                                    Ok(reply) => pending.push(async move {
                                        (id, reply.await.unwrap_or_else(|_| Err("Desktop notification worker stopped before replying.".into())))
                                    }),
                                    Err(error) => finish(&worker_attention, &worker_jobs, id, Err(error)),
                                }
                            }
                            prune(&worker_attention, &worker_jobs);
                            let _ = notice.try_send(());
                        }
                        Some((id, result)) = pending.next(), if !pending.is_empty() => {
                            finish(&worker_attention, &worker_jobs, id, result);
                            let _ = notice.try_send(());
                        }
                    }
                    if !open && pending.is_empty() { break; }
                }
                worker_attention.lock().unwrap_or_else(|e| e.into_inner()).error = Some("The custom-action runtime has stopped.".into());
                let _ = notice.try_send(());
            });
        })?;
        Ok(Self(Arc::new(Handle {
            jobs,
            attention,
            notices: Mutex::new(Some(notices)),
            stop: Mutex::new(Some(stop)),
            shutdown,
        })))
    }

    pub fn submit(
        &self,
        request: ActionRequest,
    ) -> io::Result<oneshot::Receiver<Result<u64, String>>> {
        self.0.jobs.submit(request)
    }

    pub fn cancel(&self, id: u64) -> io::Result<()> {
        self.0.jobs.cancel(id)
    }

    /// Refuses unfinished commands, unhanded completions and pending delivery.
    pub fn shutdown(&self) -> io::Result<oneshot::Receiver<Result<(), String>>> {
        let (reply, receiver) = oneshot::channel();
        self.0
            .shutdown
            .try_send(reply)
            .map_err(|_| io::Error::other("The custom-action runtime is busy or stopped."))?;
        Ok(receiver)
    }

    pub fn snapshot(&self) -> Snapshot {
        let state = self.0.attention.lock().unwrap_or_else(|e| e.into_inner());
        Snapshot {
            queue: self.0.jobs.snapshot(),
            pending_notifications: state.pending.len(),
            notification_errors: state.errors.clone(),
            error: state.error.clone(),
        }
    }

    /// One coalesced frontend wakeup; neither execution nor attention waits for it.
    pub fn take_notices(&self) -> Option<mpsc::Receiver<()>> {
        self.0
            .notices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }
}

fn finish(attention: &Mutex<Attention>, jobs: &ActionJobs, id: u64, result: Result<(), String>) {
    let mut state = attention.lock().unwrap_or_else(|e| e.into_inner());
    if !state.pending.remove(&id) {
        return;
    }
    if let Err(error) = result {
        if jobs.snapshot().jobs.iter().any(|job| job.id == id) {
            state.errors.insert(id, error);
        } else {
            state.error = Some(error);
        }
    }
}

fn prune(attention: &Mutex<Attention>, jobs: &ActionJobs) {
    let mut state = attention.lock().unwrap_or_else(|e| e.into_inner());
    let snapshot = jobs.snapshot();
    state
        .errors
        .retain(|id, _| snapshot.jobs.iter().any(|job| job.id == *id));
}
