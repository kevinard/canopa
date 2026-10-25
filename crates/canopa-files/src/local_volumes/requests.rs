//! Process-owned device admission and request state. No renderer or GIO object.
mod attention;
use super::{command_wire as wire, *};
use crate::local_activity::{LocalActivity, RemovalGuard, Work};
pub(super) use attention::Host as AttentionHost;
pub use attention::Snapshot as AttentionSnapshot;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuestionToken {
    request: RequestId,
    serial: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Mount,
    Remove(Removal),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Running,
    Cancelling,
    Finished,
}

#[derive(Clone, Debug)]
pub struct Prompt {
    pub token: QuestionToken,
    pub question: wire::Question,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub id: RequestId,
    pub device: DeviceToken,
    pub name: String,
    pub operation: Operation,
    pub phase: Phase,
    pub question: Option<Prompt>,
    pub outcome: Option<wire::Outcome>,
    pub error: Option<String>,
}

enum Hold {
    Work { _hold: Work },
    Removal { _hold: RemovalGuard },
}

struct Job {
    snapshot: Snapshot,
    hold: Option<Hold>,
    serial: u64,
    mounted: Option<catalog::MountToken>,
}

#[derive(Default)]
struct State {
    last: u64,
    closing: bool,
    lost: bool,
    jobs: VecDeque<Job>,
    attention: attention::Inbox,
}

pub(super) struct Requests {
    state: Mutex<State>,
    control: Arc<Control>,
    catalog: Arc<Mutex<DeviceSnapshot>>,
    activity: LocalActivity,
    notice: Mutex<mpsc::Sender<()>>,
    attention_notice: Mutex<mpsc::Sender<()>>,
    attention_notices: Mutex<Option<mpsc::Receiver<()>>>,
}

/// Clones attach to the same requests; dropping a frontend never cancels them.
#[derive(Clone)]
pub struct DeviceRequests(pub(super) Arc<Requests>);

enum Intent {
    Mount,
    Remove,
}

/// The result of explicit Open intent. The backend owns mount admission;
/// the frontend owns whether the returned location still belongs in its view.
#[derive(Clone, Debug)]
pub enum Open {
    Root(PathBuf),
    Mount(RequestId),
}

impl DeviceRequests {
    pub fn open(&self, token: DeviceToken) -> io::Result<Open> {
        let device = self.observed(token)?;
        if device.mount.is_some() {
            device.root.map(Open::Root).ok_or_else(root_changed)
        } else {
            // Admission revalidates the observation after the memory lookup.
            // A concurrent mount/change refuses instead of silently retargeting.
            self.mount(token).map(Open::Mount)
        }
    }

    /// Current prepared root for an explicitly selected catalog observation.
    /// This is memory-only; no frontend performs platform or filesystem work.
    pub fn root(&self, token: DeviceToken) -> io::Result<PathBuf> {
        let device = self.observed(token)?;
        if device.mount.is_some() {
            device.root.ok_or_else(root_changed)
        } else {
            Err(root_changed())
        }
    }

    fn observed(&self, token: DeviceToken) -> io::Result<Device> {
        let state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closing || self.0.control.stopped.load(Ordering::Acquire) {
            return Err(root_changed());
        }
        let catalog = self.0.catalog.lock().unwrap_or_else(|e| e.into_inner());
        catalog
            .devices
            .iter()
            .find(|device| catalog.status == DeviceStatus::Ready && device.token == token)
            .cloned()
            .ok_or_else(root_changed)
    }

    /// A completed mount's descriptive root is usable only while its exact
    /// prepared mount observation remains current. A remount cannot revive it.
    pub fn mounted_root(&self, id: RequestId) -> io::Result<PathBuf> {
        let state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closing || self.0.control.stopped.load(Ordering::Acquire) {
            return Err(root_changed());
        }
        let job = state
            .jobs
            .iter()
            .find(|job| job.snapshot.id == id)
            .ok_or_else(root_changed)?;
        let Some(wire::Outcome::Mounted { root }) = &job.snapshot.outcome else {
            return Err(root_changed());
        };
        let catalog = self.0.catalog.lock().unwrap_or_else(|e| e.into_inner());
        catalog
            .devices
            .iter()
            .find(|device| {
                catalog.status == DeviceStatus::Ready
                    && device.token.instance == job.snapshot.device.instance
                    && device.mount.is_some()
                    && device.mount == job.mounted
                    && device.root.as_ref() == Some(root)
            })
            .map(|_| root.clone())
            .ok_or_else(root_changed)
    }

    pub fn snapshot(&self) -> Vec<Snapshot> {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .jobs
            .iter()
            .rev()
            .map(|job| job.snapshot.clone())
            .collect()
    }
    pub fn mount(&self, device: DeviceToken) -> io::Result<RequestId> {
        self.0.submit(device, Intent::Mount)
    }
    pub fn remove(&self, device: DeviceToken) -> io::Result<RequestId> {
        self.0.submit(device, Intent::Remove)
    }

    pub fn cancel(&self, id: RequestId) -> io::Result<()> {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        let job = active(&mut state, id)?;
        if job.snapshot.phase == Phase::Cancelling {
            return Ok(());
        }
        self.0.send(id, wire::Command::Cancel)?;
        job.snapshot.phase = Phase::Cancelling;
        job.snapshot.question = None;
        job.snapshot.error = None;
        self.0.record_attention(&mut state, id);
        drop(state);
        self.0.publish();
        Ok(())
    }

    pub fn answer(&self, token: QuestionToken, answer: wire::Answer) -> io::Result<()> {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        let job = active(&mut state, token.request)?;
        if job.snapshot.phase == Phase::Cancelling
            || job
                .snapshot
                .question
                .as_ref()
                .is_none_or(|q| q.token != token)
        {
            return Err(io::Error::other(
                "This device question has changed or was cancelled.",
            ));
        }
        validate_answer(&job.snapshot.question.as_ref().unwrap().question, &answer)?;
        self.0.send(
            token.request,
            wire::Command::Answer {
                question: token.serial,
                answer,
            },
        )?;
        job.snapshot.question = None;
        job.snapshot.error = None;
        self.0.record_attention(&mut state, token.request);
        drop(state);
        self.0.publish();
        Ok(())
    }
}

fn active(state: &mut State, id: RequestId) -> io::Result<&mut Job> {
    state
        .jobs
        .iter_mut()
        .find(|job| job.snapshot.id == id && job.hold.is_some())
        .ok_or_else(|| io::Error::other("This device request has already settled."))
}

impl Requests {
    pub fn new(
        control: Arc<Control>,
        catalog: Arc<Mutex<DeviceSnapshot>>,
        activity: LocalActivity,
        notice: mpsc::Sender<()>,
    ) -> Arc<Self> {
        let (attention_notice, attention_notices) = mpsc::channel(0);
        Arc::new(Self {
            state: Mutex::new(State::default()),
            control,
            catalog,
            activity,
            notice: Mutex::new(notice),
            attention_notice: Mutex::new(attention_notice),
            attention_notices: Mutex::new(Some(attention_notices)),
        })
    }

    fn publish(&self) {
        let _ = self
            .notice
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_send(());
        let _ = self
            .attention_notice
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_send(());
    }

    fn record_attention(&self, state: &mut State, id: RequestId) {
        if let Some(job) = state.jobs.iter().find(|job| job.snapshot.id == id) {
            state.attention.record(&job.snapshot, &self.activity);
        }
    }

    fn send(&self, id: RequestId, command: wire::Command) -> io::Result<()> {
        let mut bytes = serde_json::to_vec(&wire::Request {
            version: 1,
            id: id.0,
            command,
        })
        .map_err(|_| io::Error::other("Could not prepare the device request."))?;
        if bytes.len() as u64 >= wire::INPUT_LIMIT {
            return Err(io::Error::other(
                "The device request exceeds its size limit.",
            ));
        }
        bytes.push(b'\n');
        let input = self.control.input.lock().unwrap_or_else(|e| e.into_inner());
        input
            .as_ref()
            .ok_or_else(|| io::Error::other("The device helper is unavailable."))?
            .try_send(bytes)
            .map_err(|_| io::Error::other("The device helper cannot accept more requests."))
    }

    fn submit(&self, token: DeviceToken, intent: Intent) -> io::Result<RequestId> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.closing || self.control.stopped.load(Ordering::Acquire) {
            return Err(io::Error::other("The device runtime is shutting down."));
        }
        if state.jobs.iter().any(|job| job.hold.is_some()) {
            return Err(io::Error::other(
                "Another device request is still in progress.",
            ));
        }
        if !state.attention.has_capacity() {
            return Err(io::Error::other(
                "Device notifications are still pending. Wait for their delivery before starting more device work.",
            ));
        }
        let catalog = self.catalog.lock().unwrap_or_else(|e| e.into_inner());
        let device = catalog
            .devices
            .iter()
            .find(|device| catalog.status == DeviceStatus::Ready && device.token == token)
            .cloned()
            .ok_or_else(|| {
                io::Error::other("This device observation has changed. Refresh and try again.")
            })?;
        let (operation, command) = if matches!(intent, Intent::Remove) {
            let removal = device
                .removal
                .ok_or_else(|| io::Error::other("This device cannot currently be removed."))?;
            (
                Operation::Remove(removal),
                wire::Command::Remove {
                    instance: token.instance,
                    generation: token.revision,
                    removal,
                },
            )
        } else {
            if !device.can_mount {
                return Err(io::Error::other("This device cannot currently be mounted."));
            }
            (
                Operation::Mount,
                wire::Command::Mount {
                    instance: token.instance,
                    generation: token.revision,
                },
            )
        };
        drop(catalog);
        let hold = match operation {
            Operation::Remove(_) => Hold::Removal {
                _hold: self.activity.try_remove()?,
            },
            Operation::Mount => {
                let work = self.activity.work();
                if !work.ready() {
                    return Err(io::Error::other("A device removal is still in progress."));
                }
                Hold::Work { _hold: work }
            }
        };
        let id = RequestId(
            state
                .last
                .checked_add(1)
                .ok_or_else(|| io::Error::other("Device request counter exhausted."))?,
        );
        state.last = id.0;
        // Install the hold under the event-reader lock before sending. A fast
        // native callback cannot retire a request that admission has not stored.
        state.jobs.push_back(Job {
            snapshot: Snapshot {
                id,
                device: token,
                name: device.name,
                operation,
                phase: Phase::Running,
                question: None,
                outcome: None,
                error: None,
            },
            hold: Some(hold),
            serial: 0,
            mounted: None,
        });
        if let Err(error) = self.send(id, command) {
            state.jobs.pop_back();
            return Err(error);
        }
        while state.jobs.len() > 32 {
            state.jobs.pop_front();
        }
        drop(state);
        self.publish();
        Ok(id)
    }

    pub fn update(&self, event: wire::Event) -> io::Result<()> {
        if event.version != 1 || event.id == 0 {
            return Err(protocol_error());
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let Some(job) = state
            .jobs
            .iter_mut()
            .find(|job| job.snapshot.id.0 == event.id)
        else {
            return Err(protocol_error());
        };
        if job.hold.is_none() {
            // A Cancel/Answer can cross the native completion in the two pipes.
            return match event.event {
                wire::Update::Refused { .. } => Ok(()),
                _ => Err(protocol_error()),
            };
        }
        let mut retired = None;
        let mut attention_changed = true;
        match event.event {
            wire::Update::Question { question, prompt } => {
                if let Some(prompt) = prompt {
                    if question <= job.serial || !valid_question(&prompt) {
                        return Err(protocol_error());
                    }
                    job.serial = question;
                    if job.snapshot.phase != Phase::Cancelling {
                        job.snapshot.question = Some(Prompt {
                            token: QuestionToken {
                                request: job.snapshot.id,
                                serial: question,
                            },
                            question: prompt,
                        });
                    }
                } else {
                    if question == 0 || question != job.serial {
                        return Err(protocol_error());
                    }
                    job.snapshot.question = None;
                }
            }
            wire::Update::Refused { message, terminal } => {
                if message.len() > 16384 {
                    return Err(protocol_error());
                }
                if terminal {
                    retired = settle(job, wire::Outcome::Failed { message });
                } else {
                    job.snapshot.error = Some(message);
                    attention_changed = false;
                }
            }
            wire::Update::Finished { outcome } => {
                match (&job.snapshot.operation, &outcome) {
                    (Operation::Mount, wire::Outcome::Mounted { root }) => {
                        let catalog = self.catalog.lock().unwrap_or_else(|e| e.into_inner());
                        let device = catalog.devices.iter().find(|d| {
                            catalog.status == DeviceStatus::Ready
                                && d.mount.is_some()
                                && d.token.instance == job.snapshot.device.instance
                                && d.root.as_ref() == Some(root)
                        });
                        if !root.is_absolute() || device.is_none() {
                            return Err(protocol_error());
                        }
                        job.mounted = device.and_then(|device| device.mount);
                    }
                    (Operation::Remove(_), wire::Outcome::Removed)
                    | (_, wire::Outcome::Cancelled) => {}
                    (_, wire::Outcome::Failed { message }) if message.len() <= 16384 => {}
                    _ => return Err(protocol_error()),
                }
                retired = settle(job, outcome);
            }
        }
        if attention_changed {
            self.record_attention(&mut state, RequestId(event.id));
        }
        // Notification responsibility is installed before actual device work
        // releases its hold, including a completion with no browser attached.
        drop(retired);
        drop(state);
        self.publish();
        Ok(())
    }

    /// Call only after the exact helper and its input writer have retired.
    pub fn lost(&self, message: &str) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.closing = true;
        state.lost = true;
        let mut retired = Vec::new();
        let mut completed = Vec::new();
        for job in &mut state.jobs {
            if job.hold.is_some() {
                completed.push(job.snapshot.id);
                retired.push(settle(
                    job,
                    wire::Outcome::Uncertain {
                        message: format!(
                            "The device helper stopped before confirming this request. The device may have changed. {message}"
                        ),
                    },
                ));
            }
        }
        for id in completed {
            self.record_attention(&mut state, id);
        }
        drop(retired);
        drop(state);
        self.publish();
    }

    pub fn close(self: &Arc<Self>) -> io::Result<Closing> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.jobs.iter().any(|job| job.hold.is_some())
            || state.attention.pending() > 0
            || state.closing && !state.lost && !self.control.stopped.load(Ordering::Acquire)
        {
            return Err(io::Error::other(
                "Device work is still active or already shutting down.",
            ));
        }
        state.closing = true;
        Ok(Closing(self.clone()))
    }
}

pub(super) struct Closing(Arc<Requests>);
impl Drop for Closing {
    fn drop(&mut self) {
        if !self.0.control.stopped.load(Ordering::Acquire) {
            let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
            if !state.lost {
                state.closing = false;
            }
        }
    }
}

fn settle(job: &mut Job, outcome: wire::Outcome) -> Option<Hold> {
    job.snapshot.phase = Phase::Finished;
    job.snapshot.question = None;
    job.snapshot.error = None;
    job.snapshot.outcome = Some(outcome);
    job.hold.take()
}

fn protocol_error() -> io::Error {
    io::Error::other("Invalid device helper response.")
}

fn root_changed() -> io::Error {
    io::Error::other(
        "This device location is no longer current. Select the device again to open it.",
    )
}

fn valid_question(question: &wire::Question) -> bool {
    match question {
        wire::Question::Password {
            message,
            username,
            domain,
            ..
        } => message.len() <= 16384 && username.len() <= 4096 && domain.len() <= 4096,
        wire::Question::Choice {
            message, choices, ..
        } => {
            message.len() <= 16384
                && !choices.is_empty()
                && choices.len() <= 64
                && choices.iter().all(|choice| choice.len() <= 4096)
        }
    }
}

fn validate_answer(question: &wire::Question, answer: &wire::Answer) -> io::Result<()> {
    let valid = match (question, answer) {
        (
            wire::Question::Password { flags, .. },
            wire::Answer::Password {
                username,
                domain,
                password,
                anonymous,
                pim,
                hidden_volume,
                system_volume,
            },
        ) => {
            [username, domain, password]
                .iter()
                .all(|s| s.len() <= 4096 && !s.contains('\0'))
                && (!anonymous || flags & gio::AskPasswordFlags::ANONYMOUS_SUPPORTED.bits() != 0)
                && (*pim == 0 && !hidden_volume && !system_volume
                    || flags & gio::AskPasswordFlags::TCRYPT.bits() != 0)
        }
        (wire::Question::Choice { choices, .. }, wire::Answer::Choice { choice }) => {
            *choice < choices.len()
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(io::Error::other(
            "The answer does not match the current device question.",
        ))
    }
}
