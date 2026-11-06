//! One bounded event reader and one coalescing filesystem worker per backend.
use super::{AppStates, FRESHNESS, MAX_WINDOWS, Observation, Unavailable, process::Processes};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    sync::watch,
    task::JoinHandle,
};

const LINE_BYTES: usize = 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct WindowKey(pub(super) u64, pub(super) u64);
#[derive(Clone)]
pub(super) struct Window {
    pub key: WindowKey,
    pub pid: Option<i32>,
    pub focused: bool,
}
pub(super) struct Job {
    pub revision: u64,
    pub windows: Vec<Window>,
    pub queued: Instant,
}

#[derive(Deserialize)]
struct WireWindow {
    id: u64,
    pid: Option<i32>,
    is_focused: bool,
}
#[derive(Deserialize)]
enum Event {
    WindowsChanged { windows: Vec<WireWindow> },
    WindowOpenedOrChanged { window: WireWindow },
    WindowClosed { id: u64 },
    WindowFocusChanged { id: Option<u64> },
}
fn event(line: &[u8]) -> Result<Option<Event>, Unavailable> {
    let value: serde_json::Value =
        serde_json::from_slice(line).map_err(|_| Unavailable::InvalidInput)?;
    let object = value
        .as_object()
        .filter(|o| o.len() == 1)
        .ok_or(Unavailable::InvalidInput)?;
    let key = object.keys().next().expect("one event");
    if !matches!(
        key.as_str(),
        "WindowsChanged" | "WindowOpenedOrChanged" | "WindowClosed" | "WindowFocusChanged"
    ) {
        return Ok(None);
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(|_| Unavailable::InvalidInput)
}

#[derive(Default)]
struct Windows {
    epoch: u64,
    birth: u64,
    ready: bool,
    windows: BTreeMap<u64, Window>,
}
impl Windows {
    fn install(&mut self, window: WireWindow) -> Result<bool, Unavailable> {
        if window.pid.is_some_and(|pid| pid <= 0) {
            return Err(Unavailable::InvalidInput);
        }
        let changed = self
            .windows
            .get(&window.id)
            .is_none_or(|old| old.pid != window.pid || old.focused != window.is_focused);
        if !changed {
            return Ok(false);
        }
        let key = if let Some(old) = self
            .windows
            .get(&window.id)
            .filter(|old| old.pid == window.pid)
        {
            old.key
        } else {
            if !self.windows.contains_key(&window.id) && self.windows.len() >= MAX_WINDOWS {
                return Err(Unavailable::Capacity);
            }
            self.birth = self.birth.wrapping_add(1);
            WindowKey(self.epoch, self.birth)
        };
        if window.is_focused {
            for item in self.windows.values_mut() {
                item.focused = false;
            }
        }
        self.windows.insert(
            window.id,
            Window {
                key,
                pid: window.pid,
                focused: window.is_focused,
            },
        );
        Ok(true)
    }

    fn apply(&mut self, event: Event) -> Result<bool, Unavailable> {
        if let Event::WindowsChanged { windows } = event {
            if windows.len() > MAX_WINDOWS {
                return Err(Unavailable::Capacity);
            }
            let ids: BTreeSet<_> = windows.iter().map(|w| w.id).collect();
            if ids.len() != windows.len() || windows.iter().filter(|w| w.is_focused).count() > 1 {
                return Err(Unavailable::InvalidInput);
            }
            self.windows.clear();
            for window in windows {
                self.install(window)?;
            }
            self.ready = true;
            return Ok(true);
        }
        if !self.ready {
            return Err(Unavailable::InvalidInput);
        }
        match event {
            Event::WindowOpenedOrChanged { window } => self.install(window),
            Event::WindowClosed { id } => Ok(self.windows.remove(&id).is_some()),
            Event::WindowFocusChanged { id } => {
                if id.is_some_and(|id| !self.windows.contains_key(&id)) {
                    return Err(Unavailable::InvalidInput);
                }
                let mut changed = false;
                for (current, window) in &mut self.windows {
                    let focused = Some(*current) == id;
                    changed |= window.focused != focused;
                    window.focused = focused;
                }
                Ok(changed)
            }
            Event::WindowsChanged { .. } => unreachable!(),
        }
    }
}

struct Publisher {
    revision: Mutex<u64>,
    closed: AtomicBool,
    state: watch::Sender<Observation>,
}
impl Publisher {
    fn invalidate(&self, reason: Unavailable) -> u64 {
        let mut revision = self.revision.lock().expect("observation revision");
        if self.closed.load(Ordering::Acquire) {
            return *revision;
        }
        *revision = revision.wrapping_add(1);
        self.state.send_if_modified(|current| {
            if current.result == Err(reason) {
                return false;
            }
            *current = Observation::unavailable(reason);
            true
        });
        *revision
    }
    fn publish(&self, job: &Job, result: Result<AppStates, Unavailable>) {
        let revision = self.revision.lock().expect("observation revision");
        if self.closed.load(Ordering::Acquire) || *revision != job.revision {
            return;
        }
        let result = if job.queued.elapsed() > FRESHNESS {
            Err(Unavailable::Stale)
        } else {
            result
        };
        let next = Observation {
            result: result.map(Arc::new),
            at: Instant::now(),
        };
        self.state.send_if_modified(|current| {
            let changed = current.result != next.result;
            *current = next;
            changed
        });
    }
    fn expire(&self) {
        let _revision = self.revision.lock().expect("observation revision");
        if self.closed.load(Ordering::Acquire) {
            return;
        }
        self.state.send_if_modified(|current| {
            if current.result.is_ok() && current.at.elapsed() > FRESHNESS {
                *current = Observation::unavailable(Unavailable::Stale);
                return true;
            }
            false
        });
    }
    fn close(&self) {
        let mut revision = self.revision.lock().expect("observation revision");
        self.closed.store(true, Ordering::Release);
        *revision = revision.wrapping_add(1);
        self.state
            .send_replace(Observation::unavailable(Unavailable::Shutdown));
    }
}

#[derive(Default)]
struct Inbox {
    state: Mutex<(Option<Job>, bool)>,
    wake: Condvar,
}
impl Inbox {
    fn submit(&self, job: Job) {
        let mut state = self.state.lock().expect("identity inbox");
        if !state.1 {
            state.0 = Some(job);
            self.wake.notify_one();
        }
    }
    fn next(&self) -> Option<Job> {
        let mut state = self.state.lock().expect("identity inbox");
        loop {
            if state.1 {
                return None;
            }
            if let Some(job) = state.0.take() {
                return Some(job);
            }
            state = self.wake.wait(state).expect("identity inbox");
        }
    }
    fn close(&self) {
        let mut state = self.state.lock().expect("identity inbox");
        *state = (None, true);
        self.wake.notify_one();
    }
}

pub struct Observer {
    publisher: Arc<Publisher>,
    inbox: Arc<Inbox>,
    task: JoinHandle<()>,
}
impl Observer {
    /// Requires an active Tokio runtime. No compositor actions are issued.
    pub fn spawn(path: Option<PathBuf>) -> io::Result<Self> {
        let runtime = tokio::runtime::Handle::try_current().map_err(io::Error::other)?;
        let (state, _) = watch::channel(Observation::unavailable(Unavailable::Connecting));
        let publisher = Arc::new(Publisher {
            revision: Mutex::new(0),
            closed: AtomicBool::new(false),
            state,
        });
        let inbox = Arc::new(Inbox::default());
        let output = publisher.clone();
        let incoming = inbox.clone();
        std::thread::Builder::new()
            .name("portal-app-identity".into())
            .spawn(move || {
                let mut processes = Processes::default();
                while let Some(job) = incoming.next() {
                    let result = processes.project(&job);
                    output.publish(&job, result);
                }
            })?;
        let task = runtime.spawn(run(path, publisher.clone(), inbox.clone()));
        Ok(Self {
            publisher,
            inbox,
            task,
        })
    }
    pub fn subscribe(&self) -> watch::Receiver<Observation> {
        self.publisher.state.subscribe()
    }
}
impl Drop for Observer {
    fn drop(&mut self) {
        self.publisher.close();
        self.task.abort();
        self.inbox.close();
    }
}

async fn line(
    reader: &mut (impl AsyncBufRead + Unpin),
    bytes: &mut Vec<u8>,
    started: &mut Option<tokio::time::Instant>,
) -> io::Result<()> {
    loop {
        let buffer = reader.fill_buf().await?;
        if buffer.is_empty() {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        started.get_or_insert_with(tokio::time::Instant::now);
        let newline = buffer.iter().position(|b| *b == b'\n');
        let count = newline.map_or(buffer.len(), |n| n + 1);
        if bytes.len() + count > LINE_BYTES {
            return Err(io::Error::other("event line over budget"));
        }
        bytes.extend_from_slice(&buffer[..count]);
        reader.consume(count);
        if newline.is_some() {
            return Ok(());
        }
    }
}

fn enqueue(windows: &Windows, output: &Publisher, inbox: &Inbox, changed: bool) {
    let revision = if changed {
        output.invalidate(Unavailable::Updating)
    } else {
        *output.revision.lock().expect("observation revision")
    };
    inbox.submit(Job {
        revision,
        windows: windows.windows.values().cloned().collect(),
        queued: Instant::now(),
    });
}

async fn connected(
    stream: UnixStream,
    epoch: u64,
    output: &Publisher,
    inbox: &Inbox,
) -> Result<(), Unavailable> {
    let mut reader = BufReader::new(stream);
    reader
        .get_mut()
        .write_all(b"\"EventStream\"\n")
        .await
        .map_err(|_| Unavailable::Disconnected)?;
    let mut bytes = Vec::new();
    let mut started = None;
    tokio::time::timeout(CONNECT_TIMEOUT, line(&mut reader, &mut bytes, &mut started))
        .await
        .map_err(|_| Unavailable::Disconnected)?
        .map_err(|_| Unavailable::InvalidInput)?;
    if serde_json::from_slice::<serde_json::Value>(&bytes).ok()
        != Some(serde_json::json!({"Ok":"Handled"}))
    {
        return Err(Unavailable::InvalidInput);
    }
    bytes.clear();
    started = None;
    let mut windows = Windows {
        epoch,
        ..Default::default()
    };
    let mut refresh = tokio::time::interval(Duration::from_secs(1));
    refresh.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let initial_deadline = tokio::time::Instant::now() + CONNECT_TIMEOUT;
    loop {
        let partial_deadline = started.map(|at| at + CONNECT_TIMEOUT);
        tokio::select! {
            result = line(&mut reader, &mut bytes, &mut started) => {
                result.map_err(|_| Unavailable::Disconnected)?;
                if let Some(event) = event(&bytes)? && windows.apply(event)? { enqueue(&windows, output, inbox, true); }
                bytes.clear();
                started = None;
            }
            _ = refresh.tick(), if windows.ready => { output.expire(); enqueue(&windows, output, inbox, false); },
            _ = tokio::time::sleep_until(initial_deadline), if !windows.ready => return Err(Unavailable::InvalidInput),
            _ = tokio::time::sleep_until(partial_deadline.unwrap_or(initial_deadline)), if partial_deadline.is_some() => return Err(Unavailable::InvalidInput),
        }
    }
}

async fn run(path: Option<PathBuf>, output: Arc<Publisher>, inbox: Arc<Inbox>) {
    let Some(path) = path else {
        output.invalidate(Unavailable::Disconnected);
        return;
    };
    let mut epoch = 0u64;
    let mut delay = Duration::from_millis(200);
    loop {
        epoch = epoch.wrapping_add(1);
        let result = match tokio::time::timeout(CONNECT_TIMEOUT, UnixStream::connect(&path)).await {
            Ok(Ok(stream)) => connected(stream, epoch, &output, &inbox).await,
            _ => Err(Unavailable::Disconnected),
        };
        output.invalidate(result.err().unwrap_or(Unavailable::Disconnected));
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(Duration::from_secs(5));
    }
}

#[cfg(test)]
mod tests;
