//! Device attention belongs to the owning runtime, never its browser windows.
use super::*;
use crate::desktop_notifications::Worker;
use futures_channel::oneshot;
use futures_util::{StreamExt, stream::FuturesUnordered};

const DELIVERY_LIMIT: usize = 17;
const PENDING_LIMIT: usize = 1024;

struct Notice {
    id: RequestId,
    title: String,
    body: String,
    receipt: Arc<()>,
}

struct Pending {
    notice: Notice,
    hold: Work,
}

#[derive(Default)]
pub(super) struct Inbox {
    claimed: bool,
    pending: BTreeMap<RequestId, Pending>,
    handed: Vec<(Arc<()>, Work)>,
}

impl Inbox {
    pub fn pending(&self) -> usize {
        self.pending.len() + self.handed.len()
    }
    pub fn has_capacity(&self) -> bool {
        self.pending() < PENDING_LIMIT
    }

    pub fn record(&mut self, job: &super::Snapshot, activity: &LocalActivity) {
        if !self.claimed {
            return;
        }
        let (title, body) = if job.question.is_some() {
            (
                "Device needs a decision".into(),
                format!("{} · Open Operations to review.", job.name),
            )
        } else if let Some(outcome) = &job.outcome {
            let title = match job.operation {
                Operation::Mount => format!("Mount {}", job.name),
                Operation::Remove(_) => format!("Remove {}", job.name),
                Operation::Connect => format!("Connect to {}", job.name),
            };
            let body = match outcome {
                wire::Outcome::Mounted { .. } => "Device mounted.".into(),
                wire::Outcome::Connected { .. } => "Connected.".into(),
                wire::Outcome::ConnectedRemote { .. } => {
                    "Connected, but GVFS offers no local folder for it.".into()
                }
                wire::Outcome::Removed => match job.operation {
                    Operation::Remove(Removal::PowerOff) => "Device powered off.",
                    Operation::Remove(Removal::Eject) => "Device ejected.",
                    _ => "Device unmounted.",
                }
                .into(),
                wire::Outcome::Cancelled => "Cancelled.".into(),
                wire::Outcome::Failed { message } | wire::Outcome::Uncertain { message } => {
                    message.clone()
                }
            };
            (title, body)
        } else {
            self.pending.remove(&job.id);
            return;
        };
        self.pending.insert(
            job.id,
            Pending {
                notice: Notice {
                    id: job.id,
                    title,
                    body,
                    receipt: Arc::new(()),
                },
                hold: activity.work(),
            },
        );
    }

    fn take(&mut self) -> Vec<Notice> {
        let mut ready = Vec::new();
        for _ in self.handed.len()..DELIVERY_LIMIT {
            let Some((_, pending)) = self.pending.pop_first() else {
                break;
            };
            self.handed
                .push((pending.notice.receipt.clone(), pending.hold));
            ready.push(pending.notice);
        }
        ready
    }

    fn acknowledge(&mut self, notice: Notice) {
        if let Some(index) = self
            .handed
            .iter()
            .position(|(receipt, _)| Arc::ptr_eq(receipt, &notice.receipt))
        {
            self.handed.swap_remove(index);
        }
    }
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub requests: Vec<super::Snapshot>,
    pub pending_notifications: usize,
    pub notification_errors: BTreeMap<RequestId, String>,
    pub error: Option<String>,
}

#[derive(Default)]
struct Failures {
    jobs: BTreeMap<RequestId, String>,
    latest: Option<String>,
}

pub(crate) struct Host {
    requests: DeviceRequests,
    failures: Arc<Mutex<Failures>>,
    stop: Option<oneshot::Sender<()>>,
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

impl Host {
    pub fn start(
        requests: DeviceRequests,
        notifications: Result<Worker, String>,
    ) -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let mut state = requests.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.attention.claimed {
            return Err(io::Error::other("Device attention already has an owner."));
        }
        let mut changes = requests
            .0
            .attention_notices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .ok_or_else(|| io::Error::other("Device attention notices already claimed."))?;
        state.attention.claimed = true;
        drop(state);
        let failures = Arc::new(Mutex::new(Failures::default()));
        let worker_failures = failures.clone();
        let worker_requests = requests.clone();
        let (stop, mut stopped) = oneshot::channel();
        std::thread::Builder::new().name("files-device-attention".into()).spawn(move || {
            runtime.block_on(async move {
                let mut pending = FuturesUnordered::new();
                let mut open = true;
                loop {
                    let ready = worker_requests.0.state.lock().unwrap_or_else(|e| e.into_inner()).attention.take();
                    let mut refused = false;
                    for notice in ready {
                        let result = notifications.as_ref().map_err(Clone::clone).and_then(|worker| {
                            worker.send(notice.title.clone(), notice.body.clone()).map_err(|error| error.to_string())
                        });
                        match result {
                            Ok(reply) => pending.push(async move {
                                (notice, reply.await.unwrap_or_else(|_| Err("Desktop notification worker stopped before replying.".into())))
                            }),
                            Err(error) => {
                                finish(&worker_failures, &worker_requests, notice, Err(error));
                                refused = true;
                            }
                        }
                    }
                    if !open && pending.is_empty() && !refused { break; }
                    tokio::select! {
                        _ = &mut stopped => return,
                        _ = std::future::ready(()), if refused => {},
                        change = changes.next(), if open => { open = change.is_some(); },
                        Some((notice, result)) = pending.next(), if !pending.is_empty() => {
                            finish(&worker_failures, &worker_requests, notice, result);
                        }
                    }
                }
            });
        })?;
        Ok(Self {
            requests,
            failures,
            stop: Some(stop),
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        let failures = self.failures.lock().unwrap_or_else(|e| e.into_inner());
        let state = self
            .requests
            .0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        Snapshot {
            requests: state
                .jobs
                .iter()
                .rev()
                .map(|job| job.snapshot.clone())
                .collect(),
            pending_notifications: state.attention.pending(),
            notification_errors: failures.jobs.clone(),
            error: failures.latest.clone(),
        }
    }
}

fn finish(
    failures: &Mutex<Failures>,
    requests: &DeviceRequests,
    notice: Notice,
    result: Result<(), String>,
) {
    let mut failures = failures.lock().unwrap_or_else(|e| e.into_inner());
    let mut state = requests.0.state.lock().unwrap_or_else(|e| e.into_inner());
    failures
        .jobs
        .retain(|id, _| state.jobs.iter().any(|job| job.snapshot.id == *id));
    if let Err(error) = result {
        if state.jobs.iter().any(|job| job.snapshot.id == notice.id) {
            failures.jobs.insert(notice.id, error);
        } else {
            failures.latest = Some(error);
        }
    }
    // The visible failure is recorded before retiring the core-owned hold.
    state.attention.acknowledge(notice);
    drop(state);
    drop(failures);
    requests.0.publish();
}
