//! GIO stays on the main thread of a helper process. A bounded plain-data pipe
//! feeds the core Storage observer without initializing a GUI.
mod catalog;
mod command_wire;
mod helper_commands;
mod interaction;
#[cfg(test)]
mod observation_tests;
mod requests;
use crate::storage::Storage;
pub use catalog::{Device, DeviceId, DeviceSnapshot, DeviceStatus, DeviceToken, Removal};
pub use command_wire::{
    Answer as DeviceAnswer, Outcome as DeviceOutcome, PasswordFields, Question as DeviceQuestion,
};
use futures_channel::mpsc;
use futures_util::FutureExt;
use gio::prelude::*;
pub use requests::{
    AttentionSnapshot as DeviceAttentionSnapshot, DeviceRequests, Open as DeviceOpen,
    Operation as DeviceOperation, Phase as DevicePhase, Prompt as DevicePrompt,
    QuestionToken as DeviceQuestionToken, RequestId as DeviceRequestId,
    Snapshot as DeviceRequestSnapshot,
};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    io::{self, BufRead, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

const FRAME_LIMIT: u64 = 4 * 1024 * 1024;
pub use crate::storage::Ready;

struct Operations {
    monitor: Monitor,
    jobs: crate::operation_jobs::OperationJobs,
    attention: crate::operation_attention::OperationAttention,
    device_attention: requests::AttentionHost,
}

/// Own platform observations and the queue whose startup they authorize.
/// A frontend clone observes the same lifetime without driving initialization.
#[derive(Clone)]
pub struct LocalOperations(Arc<Operations>);

impl LocalOperations {
    pub fn start(
        directory: PathBuf,
        notifications: Result<crate::desktop_notifications::Worker, String>,
    ) -> io::Result<Self> {
        Self::start_with_activity(
            directory,
            notifications,
            crate::local_activity::LocalActivity::default(),
        )
    }

    pub fn start_with_activity(
        directory: PathBuf,
        notifications: Result<crate::desktop_notifications::Worker, String>,
        activity: crate::local_activity::LocalActivity,
    ) -> io::Result<Self> {
        let (monitor, ready) =
            Monitor::start_with_activity(directory.join("volumes.json"), activity.clone())?;
        let jobs = crate::operation_jobs::OperationJobs::start_after_storage_with_activity(
            directory.join("operations.sqlite3"),
            ready,
            activity,
        )?;
        let device_attention =
            requests::AttentionHost::start(monitor.device_requests(), notifications.clone())?;
        let attention =
            crate::operation_attention::OperationAttention::start(jobs.clone(), notifications)?;
        Ok(Self(Arc::new(Operations {
            monitor,
            jobs,
            attention,
            device_attention,
        })))
    }

    pub fn jobs(&self) -> &crate::operation_jobs::OperationJobs {
        &self.0.jobs
    }

    pub fn storage_error(&self) -> Option<String> {
        self.0.monitor.error()
    }

    /// Prepared local devices, independent of any frontend or Browser lifetime.
    /// Observation tokens do not authorize filesystem work or device removal.
    pub fn devices(&self) -> DeviceSnapshot {
        self.0.monitor.snapshot()
    }

    pub fn device_requests(&self) -> DeviceRequests {
        self.0.monitor.device_requests()
    }

    pub fn device_attention_snapshot(&self) -> DeviceAttentionSnapshot {
        self.0.device_attention.snapshot()
    }

    pub fn attention_snapshot(&self) -> crate::operation_attention::Snapshot {
        self.0.attention.snapshot()
    }

    pub fn take_operation_notices(&self) -> Option<mpsc::Receiver<()>> {
        self.0.attention.take_notices()
    }

    /// Refuse unfinished filesystem work, then await both history teardown and
    /// the last platform observation/helper before instance ownership is released.
    pub async fn shutdown(&self) -> io::Result<()> {
        let _closing = self.0.monitor.requests.close()?;
        if self.0.jobs.snapshot().error.is_none() {
            self.0
                .jobs
                .shutdown()?
                .await
                .map_err(io::Error::other)?
                .map_err(io::Error::other)?;
        }
        self.0.monitor.shutdown_started().await
    }

    /// Coalesced prepared device changes and platform failures; queue changes
    /// use OperationJobs' stream. Observation is not needed for initialization.
    pub fn take_notices(&self) -> Option<mpsc::Receiver<()>> {
        self.0.monitor.take_notices()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    version: u8,
    generation: u64,
    mounts: Vec<Mount>,
    devices: Vec<catalog::WireDevice>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Mount {
    instance: u64,
    revision: u64,
    #[serde(with = "crate::path_serde")]
    path: PathBuf,
    identity: String,
}

struct Control {
    stopped: AtomicBool,
    input: Mutex<Option<std::sync::mpsc::SyncSender<Vec<u8>>>>,
}

pub struct Monitor {
    control: Arc<Control>,
    catalog: Arc<Mutex<DeviceSnapshot>>,
    requests: Arc<requests::Requests>,
    notices: Mutex<Option<mpsc::Receiver<()>>>,
    finished: futures_util::future::Shared<futures_channel::oneshot::Receiver<()>>,
}

impl Monitor {
    pub fn start(path: PathBuf) -> io::Result<(Self, std::sync::mpsc::Receiver<Ready>)> {
        Self::start_with_activity(path, crate::local_activity::LocalActivity::default())
    }

    pub fn start_with_activity(
        path: PathBuf,
        activity: crate::local_activity::LocalActivity,
    ) -> io::Result<(Self, std::sync::mpsc::Receiver<Ready>)> {
        let control = Arc::new(Control {
            stopped: AtomicBool::new(false),
            input: Mutex::new(None),
        });
        let worker = control.clone();
        let catalog = Arc::new(Mutex::new(DeviceSnapshot::default()));
        let publication = catalog.clone();
        let (mut notice, notices) = mpsc::channel(0);
        let requests =
            requests::Requests::new(control.clone(), catalog.clone(), activity, notice.clone());
        let device_requests = requests.clone();
        let (ready, receiver) = std::sync::mpsc::sync_channel(1);
        let (done, finished) = futures_channel::oneshot::channel();
        std::thread::Builder::new()
            .name("files-local-volumes".into())
            .spawn(move || {
                let mut ready = Some(ready);
                let result = observe(
                    path,
                    &worker,
                    &mut ready,
                    &publication,
                    &mut notice,
                    &device_requests,
                );
                device_requests.lost(
                    &result
                        .as_ref()
                        .err()
                        .map_or_else(|| "The helper has exited.".into(), ToString::to_string),
                );
                if !worker.stopped.load(Ordering::Acquire) {
                    let error = result.err().map_or_else(
                        || {
                            "Local volume monitoring stopped. Restart Files before making changes."
                                .into()
                        },
                        |error| error.to_string(),
                    );
                    let mut state = publication.lock().unwrap_or_else(|e| e.into_inner());
                    state.status = DeviceStatus::Failed(error.clone());
                    state.devices.clear();
                    drop(state);
                    let _ = notice.try_send(());
                    if let Some(ready) = ready {
                        let _ = ready.send(Err(error));
                    }
                } else {
                    let mut state = publication.lock().unwrap_or_else(|e| e.into_inner());
                    state.status = DeviceStatus::Stopped;
                    state.devices.clear();
                    drop(state);
                    let _ = notice.try_send(());
                }
                // observe() has completed its final Storage update and reaped
                // the exact child before an orderly owner can release its lock.
                let _ = done.send(());
            })?;
        Ok((
            Self {
                control,
                catalog,
                requests,
                notices: Mutex::new(Some(notices)),
                finished: finished.shared(),
            },
            receiver,
        ))
    }

    pub fn error(&self) -> Option<String> {
        match &self
            .catalog
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .status
        {
            DeviceStatus::Failed(error) => Some(error.clone()),
            _ => None,
        }
    }
    pub fn snapshot(&self) -> DeviceSnapshot {
        self.catalog
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn device_requests(&self) -> DeviceRequests {
        DeviceRequests(self.requests.clone())
    }
    pub fn take_notices(&self) -> Option<mpsc::Receiver<()>> {
        self.notices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    pub async fn shutdown(&self) -> io::Result<()> {
        let _closing = self.requests.close()?;
        self.shutdown_started().await
    }

    async fn shutdown_started(&self) -> io::Result<()> {
        self.stop();
        self.finished.clone().await.map_err(io::Error::other)
    }

    fn stop(&self) {
        self.control.stopped.store(true, Ordering::Release);
        // EOF stops the helper's main loop. Dropping a frontend never joins a
        // filesystem/process wait; orderly application teardown awaits finished.
        self.control
            .input
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        self.stop();
    }
}

struct ObservedMount {
    path: PathBuf,
    identity: String,
    epoch: u64,
    revision: u64,
}

type Observed = BTreeMap<u64, ObservedMount>;

fn observe(
    path: PathBuf,
    control: &Control,
    ready: &mut Option<std::sync::mpsc::SyncSender<Ready>>,
    catalog: &Mutex<DeviceSnapshot>,
    notice: &mut mpsc::Sender<()>,
    requests: &requests::Requests,
) -> io::Result<()> {
    let storage = Storage::load(path)?;
    // Pin the running executable across an installation update; do not execute
    // whichever pathname might replace it while this process is alive.
    let mut child = Command::new("/proc/self/exe")
        .arg("--internal-volume-monitor")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut pipe = child.stdin.take().expect("piped helper input");
    let (sender, commands) = std::sync::mpsc::sync_channel::<Vec<u8>>(16);
    let writer = match std::thread::Builder::new()
        .name("files-device-input".into())
        .spawn(move || -> io::Result<()> {
            while let Ok(bytes) = commands.recv() {
                pipe.write_all(&bytes)?;
                pipe.flush()?;
            }
            Ok(())
        }) {
        Ok(writer) => writer,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    {
        let mut input = control.input.lock().unwrap_or_else(|e| e.into_inner());
        if !control.stopped.load(Ordering::Acquire) {
            *input = Some(sender);
        } else {
            drop(sender);
        }
    }
    let mut reader = io::BufReader::new(child.stdout.take().expect("piped helper output"));
    let mut observed = Observed::new();
    let result = read_observations(
        &mut reader,
        &storage,
        &mut observed,
        ready,
        catalog,
        notice,
        Some(requests),
    );
    // Invalidate navigation before retiring storage authority or awaiting child
    // teardown. Completion descriptions remain available while it drains.
    catalog.lock().unwrap_or_else(|e| e.into_inner()).status = DeviceStatus::Starting;
    let _ = notice.try_send(());
    for (_, mount) in observed {
        storage.removed(&mount.path, mount.epoch);
    }
    storage.observer_lost();
    drop(reader);
    control
        .input
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    let waited = child.wait();
    let written = writer
        .join()
        .map_err(|_| io::Error::other("The device input writer stopped unexpectedly."))
        .and_then(|result| result);
    result?;
    written?;
    let status = waited?;
    if !status.success() {
        return Err(io::Error::other(
            "The local volume monitor stopped unexpectedly.",
        ));
    }
    Ok(())
}

fn read_observations(
    reader: &mut impl BufRead,
    storage: &Arc<Storage>,
    previous: &mut Observed,
    ready: &mut Option<std::sync::mpsc::SyncSender<Ready>>,
    catalog: &Mutex<DeviceSnapshot>,
    notice: &mut mpsc::Sender<()>,
    requests: Option<&requests::Requests>,
) -> io::Result<()> {
    let mut generation = 0;
    let mut device_serial = catalog
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .devices
        .iter()
        .map(|device| device.id.0)
        .max()
        .unwrap_or(0);
    loop {
        let mut line = Vec::new();
        let count = reader.take(FRAME_LIMIT + 1).read_until(b'\n', &mut line)?;
        if count == 0 {
            return Ok(());
        }
        if count as u64 > FRAME_LIMIT || line.last() != Some(&b'\n') {
            return Err(io::Error::other("Invalid local volume monitor frame."));
        }
        let value: serde_json::Value = serde_json::from_slice(&line)
            .map_err(|_| io::Error::other("Invalid local volume response."))?;
        if value.get("event").is_some() {
            let event = serde_json::from_value(value)
                .map_err(|_| io::Error::other("Invalid device helper response."))?;
            requests
                .ok_or_else(|| io::Error::other("Unexpected device helper response."))?
                .update(event)?;
            continue;
        }
        let frame: Frame = serde_json::from_value(value).map_err(io::Error::other)?;
        let mut instances = BTreeSet::new();
        let mut paths = BTreeSet::new();
        if frame.version != 2
            || frame.generation <= generation
            || frame.mounts.len() > 1024
            || frame.devices.len() > 1024
            || frame.mounts.iter().any(|mount| {
                mount.instance == 0
                    || mount.revision == 0
                    || mount.revision > frame.generation
                    || !mount.path.is_absolute()
                    || !instances.insert(mount.instance)
                    || !paths.insert(mount.path.clone())
            })
        {
            return Err(io::Error::other(
                "Unsupported or invalid local volume observation.",
            ));
        }
        let mut devices = BTreeSet::new();
        let mut device_mounts = BTreeSet::new();
        if frame.devices.iter().any(|device| {
            device.instance == 0
                || device.name.len() > 16 * 1024
                || !devices.insert(device.instance)
                || device.mount.is_some_and(|instance| {
                    !instances.contains(&instance) || !device_mounts.insert(instance)
                })
        }) {
            return Err(io::Error::other("Invalid local device catalog."));
        }
        generation = frame.generation;
        // Queries must not borrow the previous Ready catalog while its storage
        // observation is being replaced on this worker.
        let identities: BTreeMap<_, _> = {
            let mut state = catalog.lock().unwrap_or_else(|e| e.into_inner());
            state.status = DeviceStatus::Starting;
            state
                .devices
                .iter()
                .map(|device| (device.token.instance, device.id))
                .collect()
        };
        let current: BTreeMap<_, _> = frame
            .mounts
            .into_iter()
            .map(|mount| (mount.instance, mount))
            .collect();
        previous.retain(|instance, mount| {
            let same = current.get(instance).is_some_and(|now| {
                now.path == mount.path
                    && now.identity == mount.identity
                    && now.revision == mount.revision
            });
            if !same {
                storage.removed(&mount.path, mount.epoch);
            }
            same
        });
        for (instance, mount) in current {
            if previous.contains_key(&instance) {
                continue;
            }
            let observation = storage.mounted(mount.path.clone(), mount.identity.clone())?;
            let epoch = observation.epoch;
            storage.prepare(observation)?;
            previous.insert(
                instance,
                ObservedMount {
                    path: mount.path,
                    identity: mount.identity,
                    epoch,
                    revision: mount.revision,
                },
            );
        }
        // Never expose a navigable catalog before its Storage observations have
        // been prepared. A failed frame leaves the last successful generation.
        let devices = frame
            .devices
            .into_iter()
            .map(|device| {
                let id = if let Some(id) = identities.get(&device.instance) {
                    *id
                } else {
                    device_serial = device_serial
                        .checked_add(1)
                        .ok_or_else(|| io::Error::other("Device identity counter exhausted."))?;
                    DeviceId(device_serial)
                };
                Ok(device.prepare(generation, previous, id))
            })
            .collect::<io::Result<Vec<_>>>()?;
        *catalog.lock().unwrap_or_else(|e| e.into_inner()) = DeviceSnapshot {
            generation,
            status: DeviceStatus::Ready,
            devices,
        };
        let _ = notice.try_send(());
        if let Some(ready) = ready.take() {
            let _ = ready.send(Ok(storage.clone()));
        }
    }
}

/// Internal binary mode: all GIO objects and callbacks remain on this process's
/// main thread, with the ordinary default context and no thread-default context.
pub fn run_helper() -> io::Result<()> {
    let context = glib::MainContext::default();
    let _owner = context.acquire().map_err(io::Error::other)?;
    let main_loop = Arc::new(glib::MainLoop::new(Some(&context), false));
    let (mut input, mut requests) = mpsc::channel(8);
    let input_failure = Arc::new(Mutex::new(None));
    let failed_input = input_failure.clone();
    std::thread::Builder::new()
        .name("files-volume-commands".into())
        .spawn(move || {
            let mut reader = io::BufReader::new(io::stdin());
            loop {
                let result = (|| {
                    let mut line = Vec::new();
                    let count = reader
                        .by_ref()
                        .take(command_wire::INPUT_LIMIT + 1)
                        .read_until(b'\n', &mut line)?;
                    if count == 0 {
                        return Ok(None);
                    }
                    if count as u64 > command_wire::INPUT_LIMIT || line.last() != Some(&b'\n') {
                        return Err(io::Error::other("Invalid bounded device command frame."));
                    }
                    // Parser diagnostics can contain input fragments. Never
                    // include them in an error that may expose a password.
                    serde_json::from_slice::<command_wire::Request>(&line)
                        .map(Some)
                        .map_err(|_| io::Error::other("Invalid device command."))
                })();
                let ended = !matches!(result, Ok(Some(_)));
                if input.try_send(result).is_err() {
                    *failed_input.lock().unwrap_or_else(|e| e.into_inner()) = Some(
                        io::Error::other("Device command input exceeded its bounded capacity."),
                    );
                    break;
                }
                if ended {
                    break;
                }
            }
        })?;
    let monitor = gio::VolumeMonitor::get();
    let revisions = Rc::new(RefCell::new(BTreeMap::new()));
    let generation = Rc::new(Cell::new(1_u64));
    let failure = Rc::new(RefCell::new(None));
    let commands = helper_commands::Commands::new(
        monitor.clone(),
        generation.clone(),
        revisions.clone(),
        main_loop.clone(),
        failure.clone(),
    );
    let command_failure = failure.clone();
    let command_input_failure = input_failure.clone();
    context.spawn_local(async move {
        use futures_util::StreamExt;
        while let Some(request) = requests.next().await {
            if command_input_failure
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_some()
            {
                commands.stop();
                return;
            }
            match request {
                Ok(Some(request)) => commands.receive(request),
                ended => {
                    if let Err(error) = ended {
                        *command_failure.borrow_mut() = Some(error);
                    }
                    commands.stop();
                    return;
                }
            }
        }
        // A bounded-input overflow also drops the sender. Drain no further
        // intent after it; cancel and await the actual platform callback.
        commands.stop();
    });
    let mut handlers = Vec::new();
    for signal in [
        "mount-added",
        "mount-removed",
        "mount-changed",
        "volume-added",
        "volume-removed",
        "volume-changed",
        "drive-connected",
        "drive-disconnected",
        "drive-changed",
    ] {
        let revisions = revisions.clone();
        let generation = generation.clone();
        let failure = failure.clone();
        let main_loop = main_loop.clone();
        handlers.push(monitor.connect_local(signal, false, move |values| {
            let result = (|| {
                let monitor = values[0]
                    .get::<gio::VolumeMonitor>()
                    .map_err(io::Error::other)?;
                if signal.starts_with("mount-") {
                    let mount = values[1].get::<gio::Mount>().map_err(io::Error::other)?;
                    revisions
                        .borrow_mut()
                        .remove(&(mount.as_ptr() as usize as u64));
                }
                let next = generation
                    .get()
                    .checked_add(1)
                    .ok_or_else(|| io::Error::other("Volume observation counter exhausted"))?;
                generation.set(next);
                publish(&monitor, next, &mut revisions.borrow_mut())
            })();
            if let Err(error) = result {
                *failure.borrow_mut() = Some(error);
                main_loop.quit();
            }
            None
        }));
    }
    publish(&monitor, 1, &mut revisions.borrow_mut())?;
    main_loop.run();
    for handler in handlers {
        monitor.disconnect(handler);
    }
    if let Some(error) = input_failure
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    {
        return Err(error);
    }
    failure.borrow_mut().take().map_or(Ok(()), Err)
}

fn publish(
    monitor: &gio::VolumeMonitor,
    generation: u64,
    revisions: &mut BTreeMap<u64, u64>,
) -> io::Result<()> {
    let mounts: Vec<_> = monitor
        .mounts()
        .into_iter()
        .filter(|mount| !mount.is_shadowed())
        .filter_map(|mount| {
            let path = mount.root().path()?;
            let instance = mount.as_ptr() as usize as u64;
            let revision = *revisions.entry(instance).or_insert(generation);
            let identity = mount
                .uuid()
                .filter(|id| !id.is_empty())
                .or_else(|| {
                    mount
                        .volume()
                        .and_then(|volume| volume.uuid())
                        .filter(|id| !id.is_empty())
                })
                .map(|uuid| format!("uuid:{uuid}"))
                .unwrap_or_else(|| format!("location:{}", mount.root().uri()));
            Some(Mount {
                instance,
                revision,
                path,
                identity,
            })
        })
        .collect();
    if mounts.len() > 1024 {
        return Err(io::Error::other("Too many local volume observations"));
    }
    let current: BTreeSet<_> = mounts.iter().map(|mount| mount.instance).collect();
    revisions.retain(|instance, _| current.contains(instance));
    let devices = catalog::collect(monitor, &current);
    if devices.len() > 1024 || devices.iter().any(|device| device.name.len() > 16 * 1024) {
        return Err(io::Error::other(
            "Local device catalog exceeds its size limit",
        ));
    }
    let mut bytes = serde_json::to_vec(&Frame {
        version: 2,
        generation,
        mounts,
        devices,
    })
    .map_err(io::Error::other)?;
    if bytes.len() as u64 >= FRAME_LIMIT {
        return Err(io::Error::other(
            "Local volume observation exceeds its size limit",
        ));
    }
    bytes.push(b'\n');
    let mut output = io::stdout().lock();
    output.write_all(&bytes)?;
    output.flush()
}
