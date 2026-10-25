//! Process-owned custom-action admission and scheduling, independent of observers.
//! Commands are never replayed or added to filesystem Undo by this module.
use crate::{
    Cancellation,
    actions::{ActionOutcome, CustomAction},
    local_activity::{LocalActivity, Work},
    storage::{self, Access, PreparedStorage},
};
use futures_channel::{mpsc as asynchronous, oneshot};
use std::{
    collections::{BTreeMap, VecDeque},
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

const RUNNING_LIMIT: usize = 4;
const UNFINISHED_LIMIT: usize = 1024;
const FINISHED_LIMIT: usize = 100;

/// The catalog action, selected Unix paths and working-folder binding captured
/// by an explicit invocation. Later configuration or navigation cannot replace it.
#[derive(Clone)]
pub struct ActionRequest {
    pub action: CustomAction,
    pub paths: Arc<Vec<PathBuf>>,
    pub directory: PathBuf,
    pub resolved_directory: PathBuf,
}

struct Execution {
    request: ActionRequest,
    placement: Option<storage::Snapshot>,
}

impl Execution {
    fn run(&self, cancellation: &Cancellation) -> io::Result<ActionOutcome> {
        let result = self.prepare_and_run(cancellation);
        match result {
            Err(_) if cancellation.is_cancelled() => Ok(cancelled_outcome()),
            result => result,
        }
    }

    fn prepare_and_run(&self, cancellation: &Cancellation) -> io::Result<ActionOutcome> {
        if cancellation.is_cancelled() {
            return Ok(cancelled_outcome());
        }
        let request = &self.request;
        if std::fs::canonicalize(&request.directory)? != request.resolved_directory {
            return Err(io::Error::other(
                "The action's original folder changed. Return to that folder and invoke the action again.",
            ));
        }
        let prepared = request.action.prepare(&request.paths, &request.directory)?;
        if let Some(placement) = &self.placement {
            let mut paths =
                Vec::with_capacity(request.paths.len().saturating_mul(2).saturating_add(1));
            paths.push(Access::folder(request.directory.clone()));
            for path in request.paths.iter() {
                paths.push(Access::entry(path.clone()));
                paths.push(Access::referent(path.clone()));
            }
            placement.check_while(&paths, || cancellation.is_cancelled())?;
        }
        prepared.run(cancellation)
    }
}

fn cancelled_outcome() -> ActionOutcome {
    ActionOutcome {
        success: false,
        exit_code: None,
        cancelled: true,
        stdout: Vec::new(),
        stderr: Vec::new(),
        stdout_truncated: false,
        stderr_truncated: false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Queued,
    WaitingForDevice,
    Running,
    Cancelling,
    Finished,
}

impl Phase {
    fn has_worker(self) -> bool {
        matches!(
            self,
            Self::WaitingForDevice | Self::Running | Self::Cancelling
        )
    }
}

#[derive(Clone, Debug)]
pub struct JobSnapshot {
    pub id: u64,
    pub title: String,
    pub directory: PathBuf,
    pub count: usize,
    pub phase: Phase,
    pub outcome: Option<Arc<Result<ActionOutcome, String>>>,
}

/// A compact completion handoff, independent of the bounded output history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionStatus {
    Success,
    Cancelled,
    Exited(Option<i32>),
    Unavailable(String),
}

impl CompletionStatus {
    pub fn description(&self) -> String {
        match self {
            Self::Success => "Finished".into(),
            Self::Cancelled => "Cancelled".into(),
            Self::Exited(Some(code)) => format!("Failed · exit code {code}"),
            Self::Exited(None) => "Command terminated".into(),
            Self::Unavailable(error) => format!("Could not run: {error}"),
        }
    }
}

impl JobSnapshot {
    pub fn completion_status(&self) -> Option<CompletionStatus> {
        self.outcome.as_deref().map(|result| match result {
            Ok(outcome) if outcome.cancelled => CompletionStatus::Cancelled,
            Ok(outcome) if outcome.success => CompletionStatus::Success,
            Ok(outcome) => CompletionStatus::Exited(outcome.exit_code),
            Err(error) => CompletionStatus::Unavailable(error.clone()),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Completion {
    pub id: u64,
    pub title: String,
    pub status: CompletionStatus,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub jobs: Vec<JobSnapshot>,
    pub pending_completions: usize,
}

impl Snapshot {
    pub fn holds_application(&self) -> bool {
        self.pending_completions > 0 || self.jobs.iter().any(|job| job.phase != Phase::Finished)
    }
}

#[derive(Clone)]
pub struct ActionJobs(Arc<Handle>);

struct Handle {
    activity: LocalActivity,
    storage: Option<PreparedStorage>,
    sender: Arc<mpsc::SyncSender<Control>>,
    snapshot: Arc<Mutex<Snapshot>>,
    completions: Arc<Mutex<Vec<Completion>>>,
    notices: Mutex<Option<asynchronous::Receiver<()>>>,
    stopped: Arc<AtomicBool>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.sender.try_send(Control::Stop);
    }
}

enum Control {
    Submit(Box<Execution>, Work, oneshot::Sender<Result<u64, String>>),
    Started(u64),
    Complete(u64, Arc<Result<ActionOutcome, String>>),
    Cancel(u64),
    Shutdown(oneshot::Sender<Result<(), String>>),
    Stop,
}

struct Job {
    work: Option<Work>,
    snapshot: JobSnapshot,
    request: Option<Execution>,
    cancellation: Cancellation,
}

struct Worker {
    jobs: BTreeMap<u64, Job>,
    queue: VecDeque<u64>,
    next: u64,
    sender: Weak<mpsc::SyncSender<Control>>,
    snapshot: Arc<Mutex<Snapshot>>,
    completions: Arc<Mutex<Vec<Completion>>>,
    notice: asynchronous::Sender<()>,
    stopped: Arc<AtomicBool>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        for job in self.jobs.values() {
            if job.snapshot.phase != Phase::Finished {
                job.cancellation.cancel();
            }
        }
    }
}

impl ActionJobs {
    /// Starts only the coordinator. Filesystem preparation and command execution
    /// run on command workers; no renderer, display or history store is required.
    pub fn start() -> io::Result<Self> {
        Self::start_with_activity(LocalActivity::default())
    }

    /// Join the owning runtime's device-removal admission.
    pub fn start_with_activity(activity: LocalActivity) -> io::Result<Self> {
        Self::start_configured(activity, None)
    }

    /// Managed hosts capture original placement at invocation, then validate
    /// current availability on command workers after any removal wait.
    pub fn start_with_storage(
        activity: LocalActivity,
        storage: PreparedStorage,
    ) -> io::Result<Self> {
        Self::start_configured(activity, Some(storage))
    }

    fn start_configured(
        activity: LocalActivity,
        storage: Option<PreparedStorage>,
    ) -> io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(UNFINISHED_LIMIT);
        let sender = Arc::new(sender);
        let snapshot = Arc::new(Mutex::new(Snapshot::default()));
        let completions = Arc::new(Mutex::new(Vec::new()));
        let (notice, notices) = asynchronous::channel(0);
        let stopped = Arc::new(AtomicBool::new(false));
        let mut worker = Worker {
            jobs: BTreeMap::new(),
            queue: VecDeque::new(),
            next: 0,
            sender: Arc::downgrade(&sender),
            snapshot: snapshot.clone(),
            completions: completions.clone(),
            notice,
            stopped: stopped.clone(),
        };
        let finished = stopped.clone();
        std::thread::Builder::new()
            .name("files-action-queue".into())
            .spawn(move || {
                let reply = worker.run(receiver);
                drop(worker);
                finished.store(true, Ordering::Release);
                if let Some(reply) = reply {
                    let _ = reply.send(Ok(()));
                }
            })?;
        Ok(Self(Arc::new(Handle {
            activity,
            storage,
            sender,
            snapshot,
            completions,
            notices: Mutex::new(Some(notices)),
            stopped,
        })))
    }

    pub fn submit(
        &self,
        request: ActionRequest,
    ) -> io::Result<oneshot::Receiver<Result<u64, String>>> {
        let (reply, receiver) = oneshot::channel();
        let work = self.0.activity.work();
        let mut placement = self
            .0
            .storage
            .as_ref()
            .map(PreparedStorage::capture)
            .transpose()?;
        if let Some(placement) = &mut placement {
            placement.bind_folder(
                request.directory.clone(),
                request.resolved_directory.clone(),
            );
        }
        self.send(Control::Submit(
            Box::new(Execution { request, placement }),
            work,
            reply,
        ))?;
        Ok(receiver)
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut snapshot = self
            .0
            .snapshot
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        snapshot.pending_completions = self
            .0
            .completions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len();
        snapshot
    }

    /// Transfers compact completion receipts to the owning host. The host must
    /// preserve their notification-delivery hold after this handoff. Finished
    /// output rows can expire without losing these receipts. Admission shares
    /// 1024 slots between unfinished commands and unhanded completions.
    pub fn take_completions(&self) -> Vec<Completion> {
        std::mem::take(
            &mut *self
                .0
                .completions
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
        )
    }

    /// One bounded coalesced wakeup; consuming it is never required for execution.
    pub fn take_notices(&self) -> Option<asynchronous::Receiver<()>> {
        self.0
            .notices
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }

    pub fn cancel(&self, id: u64) -> io::Result<()> {
        self.send(Control::Cancel(id))
    }

    /// Refuses while accepted commands are unfinished. The acknowledgement
    /// follows release of the idle coordinator; it does not kill accepted work.
    pub fn shutdown(&self) -> io::Result<oneshot::Receiver<Result<(), String>>> {
        let (reply, receiver) = oneshot::channel();
        self.send(Control::Shutdown(reply))?;
        Ok(receiver)
    }

    fn send(&self, control: Control) -> io::Result<()> {
        if self.0.stopped.load(Ordering::Acquire) {
            return Err(io::Error::other("The custom-action runtime has stopped."));
        }
        self.0.sender.try_send(control).map_err(|_| {
            io::Error::other("The custom-action queue is unavailable or full. Try again.")
        })
    }
}

impl Worker {
    fn run(
        &mut self,
        receiver: mpsc::Receiver<Control>,
    ) -> Option<oneshot::Sender<Result<(), String>>> {
        while !self.stopped.load(Ordering::Acquire) {
            let Ok(control) = receiver.recv() else {
                break;
            };
            let mut admission = None;
            match control {
                Control::Submit(request, work, reply) => {
                    let result = self
                        .admit(*request, work)
                        .map_err(|error| error.to_string());
                    admission = Some((reply, result));
                }
                Control::Started(id) => {
                    if let Some(job) = self.jobs.get_mut(&id)
                        && job.snapshot.phase == Phase::WaitingForDevice
                    {
                        job.snapshot.phase = Phase::Running;
                    }
                }
                Control::Complete(id, result) => self.complete(id, result),
                Control::Cancel(id) => self.cancel(id),
                Control::Shutdown(reply) => {
                    if self.unfinished() > 0
                        || !self
                            .completions
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .is_empty()
                    {
                        let _ = reply.send(Err("Finish or cancel accepted commands and hand off completion notices before stopping the custom-action runtime.".into()));
                    } else {
                        return Some(reply);
                    }
                }
                Control::Stop => break,
            }
            self.dispatch();
            self.trim();
            self.publish();
            // An admission receipt must never precede its visible lifecycle hold.
            if let Some((reply, result)) = admission {
                let _ = reply.send(result);
            }
        }
        None
    }

    fn admit(&mut self, execution: Execution, work: Work) -> io::Result<u64> {
        let request = &execution.request;
        if !request.directory.is_absolute()
            || !request.resolved_directory.is_absolute()
            || request.paths.iter().any(|path| !path.is_absolute())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "A custom action requires absolute selected paths and its original working-folder binding.",
            ));
        }
        request.action.validate()?;
        if self.unfinished()
            + self
                .completions
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .len()
            >= UNFINISHED_LIMIT
        {
            return Err(io::Error::other(
                "Too many unfinished commands or undelivered results. Finish or cancel work and collect its completion notices before starting another.",
            ));
        }

        let id = self
            .next
            .checked_add(1)
            .ok_or_else(|| io::Error::other("The custom-action sequence is exhausted."))?;
        self.next = id;
        self.jobs.insert(
            id,
            Job {
                work: Some(work),
                snapshot: JobSnapshot {
                    id,
                    title: request.action.name.clone(),
                    directory: request.directory.clone(),
                    count: request.paths.len(),
                    phase: Phase::Queued,
                    outcome: None,
                },
                request: Some(execution),
                cancellation: Cancellation::default(),
            },
        );
        self.queue.push_back(id);
        Ok(id)
    }

    fn cancel(&mut self, id: u64) {
        let Some(job) = self.jobs.get_mut(&id) else {
            return;
        };
        match job.snapshot.phase {
            Phase::Queued => {
                self.queue.retain(|queued| *queued != id);
                job.request = None;
                job.work = None;
                job.snapshot.phase = Phase::Finished;
            }
            Phase::WaitingForDevice | Phase::Running | Phase::Cancelling => {
                job.cancellation.cancel();
                job.snapshot.phase = Phase::Cancelling;
            }
            Phase::Finished => {}
        }
    }

    fn dispatch(&mut self) {
        while self
            .jobs
            .values()
            .filter(|job| job.snapshot.phase.has_worker())
            .count()
            < RUNNING_LIMIT
        {
            let Some(id) = self.queue.pop_front() else {
                break;
            };
            let job = self.jobs.get_mut(&id).unwrap();
            let request = job.request.take().unwrap();
            let cancellation = job.cancellation.clone();
            let work = job.work.as_ref().unwrap().clone();
            let sender = self.sender.clone();
            job.snapshot.phase = Phase::WaitingForDevice;
            let spawned = std::thread::Builder::new()
                .name("files-command".into())
                .spawn(move || {
                    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        if let Err(error) = work.wait(|| cancellation.is_cancelled()) {
                            if !cancellation.is_cancelled() {
                                return Err(error);
                            }
                            return Ok(cancelled_outcome());
                        }
                        if let Some(sender) = sender.upgrade() {
                            let _ = sender.send(Control::Started(id));
                        }
                        request.run(&cancellation)
                    }))
                    .unwrap_or_else(|_| {
                        Err(io::Error::other("The command worker stopped unexpectedly."))
                    })
                    .map_err(|error| error.to_string());
                    if let Some(sender) = sender.upgrade() {
                        let _ = sender.send(Control::Complete(id, Arc::new(outcome)));
                    }
                });
            if let Err(error) = spawned {
                self.complete(id, Arc::new(Err(error.to_string())));
            }
        }
    }

    fn unfinished(&self) -> usize {
        self.jobs
            .values()
            .filter(|job| job.snapshot.phase != Phase::Finished)
            .count()
    }

    fn complete(&mut self, id: u64, result: Arc<Result<ActionOutcome, String>>) {
        let Some(job) = self
            .jobs
            .get_mut(&id)
            .filter(|job| job.snapshot.phase.has_worker())
        else {
            return;
        };
        job.snapshot.phase = Phase::Finished;
        job.snapshot.outcome = Some(result);
        job.work = None;
        let completion = Completion {
            id,
            title: job.snapshot.title.clone(),
            status: job.snapshot.completion_status().unwrap(),
        };
        self.completions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(completion);
    }

    fn trim(&mut self) {
        let count = self
            .jobs
            .values()
            .filter(|job| job.snapshot.phase == Phase::Finished)
            .count();
        let expired: Vec<_> = self
            .jobs
            .iter()
            .filter(|(_, job)| job.snapshot.phase == Phase::Finished)
            .take(count.saturating_sub(FINISHED_LIMIT))
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            self.jobs.remove(&id);
        }
    }

    fn publish(&mut self) {
        self.snapshot
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .jobs = self.jobs.values().map(|job| job.snapshot.clone()).collect();
        let _ = self.notice.try_send(());
    }
}
