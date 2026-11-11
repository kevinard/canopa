//! Platform-owned operation attention. Execution and delivery need no renderer.
use crate::{
    desktop_notifications::Worker,
    operation_jobs::{Attention, AttentionObserver, AttentionStatus, OperationJobs},
};
use futures_channel::{mpsc, oneshot};
use futures_util::{StreamExt, stream::FuturesUnordered};
use std::{
    collections::BTreeMap,
    io,
    sync::{Arc, Mutex},
};

const ERROR_LIMIT: usize = 100;

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub queue: crate::operation_jobs::Snapshot,
    pub notification_errors: BTreeMap<u64, String>,
    pub error: Option<String>,
}

#[derive(Default)]
struct Failures {
    jobs: BTreeMap<u64, String>,
    latest: Option<String>,
}

#[derive(Clone)]
pub struct OperationAttention(Arc<Handle>);
struct Handle {
    jobs: OperationJobs,
    failures: Arc<Mutex<Failures>>,
    notices: Mutex<Option<mpsc::Receiver<()>>>,
    stop: Mutex<Option<oneshot::Sender<()>>>,
}
impl Drop for Handle {
    fn drop(&mut self) {
        if let Some(stop) = self
            .stop
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = stop.send(());
        }
    }
}

impl OperationAttention {
    /// Claims attention before exposing a queue. A known notification startup
    /// refusal remains observable without disabling admitted filesystem work.
    pub fn start(jobs: OperationJobs, notifications: Result<Worker, String>) -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let mut observer = jobs.observe_attention()?;
        let mut changes = observer
            .take_notices()
            .ok_or_else(|| io::Error::other("Operation notices already claimed."))?;
        let failures = Arc::new(Mutex::new(Failures::default()));
        let worker_failures = failures.clone();
        let worker_jobs = jobs.clone();
        let (mut notice, notices) = mpsc::channel(0);
        let (stop, mut stopped) = oneshot::channel();
        std::thread::Builder::new().name("files-operation-attention".into()).spawn(move || {
            runtime.block_on(async move {
                let mut pending = FuturesUnordered::new();
                let mut open = true;
                loop {
                    let mut refused = false;
                    // The observer limits handed receipts to seventeen and
                    // retains the core hold through every asynchronous reply.
                    for attention in observer.take_pending() {
                        let (title, body) = match &attention.status {
                            AttentionStatus::Decision { .. } => (
                                fl!("files-notification-decision"),
                                fl!("files-notification-decision-body", operation = crate::history_labels::title(&attention.title)),
                            ),
                            AttentionStatus::Outcome { detail, .. } => (crate::history_labels::title(&attention.title), detail.clone()),
                        };
                        let result = notifications.as_ref().map_err(Clone::clone).and_then(|worker| {
                            worker.send(title, body).map_err(|error| error.to_string())
                        });
                        match result {
                            Ok(reply) => pending.push(async move {
                                (attention, reply.await.unwrap_or_else(|_| Err(fl!("files-notification-worker-stopped"))))
                            }),
                            Err(error) => {
                                finish(&worker_failures, &worker_jobs, &observer, attention, Err(error));
                                refused = true;
                            }
                        }
                    }
                    if !open && pending.is_empty() && !refused { break; }
                    tokio::select! {
                        _ = &mut stopped => return,
                        // A refusal frees room for unread receipts. Stay
                        // responsive to owner drop while draining that backlog.
                        _ = std::future::ready(()), if refused => {
                            let _ = notice.try_send(());
                        }
                        change = changes.next(), if open => {
                            open = change.is_some();
                            prune(&worker_failures, &worker_jobs);
                            let _ = notice.try_send(());
                        }
                        Some((attention, result)) = pending.next(), if !pending.is_empty() => {
                            finish(&worker_failures, &worker_jobs, &observer, attention, result);
                            let _ = notice.try_send(());
                        }
                    }
                }
            });
        })?;
        Ok(Self(Arc::new(Handle {
            jobs,
            failures,
            notices: Mutex::new(Some(notices)),
            stop: Mutex::new(Some(stop)),
        })))
    }

    pub fn snapshot(&self) -> Snapshot {
        let failures = self.0.failures.lock().unwrap_or_else(|e| e.into_inner());
        Snapshot {
            queue: self.0.jobs.snapshot(),
            notification_errors: failures.jobs.clone(),
            error: failures.latest.clone(),
        }
    }

    /// One frontend wakeup; delivery and execution never wait for its receiver.
    pub fn take_notices(&self) -> Option<mpsc::Receiver<()>> {
        self.0
            .notices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }
}

fn finish(
    failures: &Mutex<Failures>,
    jobs: &OperationJobs,
    observer: &AttentionObserver,
    attention: Attention,
    result: Result<(), String>,
) {
    let mut failures = failures.lock().unwrap_or_else(|e| e.into_inner());
    if let Err(error) = result {
        if jobs
            .snapshot()
            .jobs
            .iter()
            .any(|job| job.id == attention.id)
        {
            failures.jobs.insert(attention.id, error);
            if failures.jobs.len() > ERROR_LIMIT {
                failures.latest = failures.jobs.pop_first().map(|(_, error)| error);
            }
        } else {
            failures.latest = Some(error);
        }
    }
    // Record any refusal before releasing the core's delivery hold. Snapshot
    // readers take this same failures lock before the core snapshot.
    observer.acknowledge(attention);
}

fn prune(failures: &Mutex<Failures>, jobs: &OperationJobs) {
    let mut failures = failures.lock().unwrap_or_else(|e| e.into_inner());
    let snapshot = jobs.snapshot();
    failures
        .jobs
        .retain(|id, _| snapshot.jobs.iter().any(|job| job.id == *id));
}
