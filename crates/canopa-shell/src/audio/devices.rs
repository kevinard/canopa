//! Bounded Pulse-protocol device queries and explicit, identity-bound operations.
use super::{PulseConnection, pulse};
use pulse::{callbacks::ListResult, volume::ChannelVolumes};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc,
};
use std::time::Duration;
use tokio::sync::watch;

const DEADLINE: Duration = Duration::from_secs(2);
const MAX_DEVICES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Output,
    Input,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDeviceId {
    pub kind: DeviceKind,
    pub name: String,
    pub index: u32,
    pub server_cookie: u32,
    pub session: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioDevice {
    pub id: AudioDeviceId,
    pub description: String,
    pub volume: f64,
    pub muted: bool,
    pub is_default: bool,
    channels: ChannelVolumes,
}

impl AudioDevice {
    pub fn can_increase_volume(&self) -> bool {
        self.channels.max() < pulse::volume::Volume::NORMAL
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioSnapshot {
    pub devices: Vec<AudioDevice>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceOperation {
    AdjustVolume { step: u8, up: bool },
    SetMuted(bool),
    SetDefault,
}

#[derive(Clone)]
pub struct AudioControl {
    commands: tokio::sync::mpsc::Sender<OperationRequest>,
}
struct OperationRequest {
    id: AudioDeviceId,
    operation: DeviceOperation,
    reply: tokio::sync::oneshot::Sender<Result<(), String>>,
}
impl AudioControl {
    pub async fn apply(&self, id: AudioDeviceId, operation: DeviceOperation) -> Result<(), String> {
        let (reply, response) = tokio::sync::oneshot::channel();
        self.commands
            .try_send(OperationRequest {
                id,
                operation,
                reply,
            })
            .map_err(|_| "Audio controls are busy or unavailable.".to_owned())?;
        tokio::time::timeout(Duration::from_secs(10), response)
            .await
            .map_err(|_| "Audio operation timed out.".to_owned())?
            .map_err(|_| "Audio controls are unavailable.".to_owned())?
    }
}

/// Only the most recent snapshot is retained. Dropping the receiver stops the
/// observer and its command worker, including reconnects. No audio is recorded.
pub fn monitor(server: Option<String>) -> (AudioControl, watch::Receiver<Option<AudioSnapshot>>) {
    let (tx, rx) = watch::channel(None);
    let (commands, mut requests) = tokio::sync::mpsc::channel::<OperationRequest>(8);
    std::thread::spawn(move || {
        while !tx.is_closed() {
            observe(server.as_deref(), &tx, &mut requests);
            tx.send_if_modified(|value| value.take().is_some());
            for _ in 0..20 {
                if tx.is_closed() {
                    return;
                }
                while let Ok(request) = requests.try_recv() {
                    let _ = request.reply.send(Err("Audio is unavailable.".into()));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    });
    (AudioControl { commands }, rx)
}

fn observe(
    server: Option<&str>,
    tx: &watch::Sender<Option<AudioSnapshot>>,
    requests: &mut tokio::sync::mpsc::Receiver<OperationRequest>,
) {
    let Ok(mut connection) = PulseConnection::connect(server, || tx.is_closed()) else {
        return;
    };
    static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
    let session = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
    let dirty = Arc::new(AtomicBool::new(true));
    let changed = dirty.clone();
    connection.mainloop.lock();
    connection
        .context
        .borrow_mut()
        .set_subscribe_callback(Some(Box::new(move |_, _, _| {
            changed.store(true, Ordering::Release);
        })));
    let (ready_tx, ready_rx) = mpsc::channel();
    use pulse::context::subscribe::InterestMaskSet;
    connection.context.borrow_mut().subscribe(
        InterestMaskSet::SINK | InterestMaskSet::SOURCE | InterestMaskSet::SERVER,
        move |success| {
            let _ = ready_tx.send(success);
        },
    );
    connection.mainloop.unlock();
    if ready_rx.recv_timeout(DEADLINE) != Ok(true) {
        return;
    }
    while !tx.is_closed() {
        if let Ok(request) = requests.try_recv()
            && !request.reply.is_closed()
        {
            let result = apply_to(&mut connection, session, &request.id, request.operation);
            let _ = request.reply.send(result);
            dirty.store(true, Ordering::Release);
        }
        if dirty.swap(false, Ordering::AcqRel) {
            let Ok(snapshot) = snapshot(&mut connection, session) else {
                return;
            };
            // Do not publish a query overtaken by another server change.
            if !dirty.load(Ordering::Acquire) {
                tx.send_if_modified(|value| {
                    if value.as_ref() == Some(&snapshot) {
                        false
                    } else {
                        *value = Some(snapshot);
                        true
                    }
                });
            }
        }
        connection.mainloop.lock();
        let state = connection.context.borrow().get_state();
        connection.mainloop.unlock();
        if state != pulse::context::State::Ready {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn receive<T>(rx: mpsc::Receiver<T>) -> Result<T, String> {
    rx.recv_timeout(DEADLINE)
        .map_err(|_| "Audio server did not respond.".into())
}

fn snapshot(connection: &mut PulseConnection, session: u64) -> Result<AudioSnapshot, String> {
    let (tx, rx) = mpsc::channel();
    connection.mainloop.lock();
    connection
        .context
        .borrow()
        .introspect()
        .get_server_info(move |server| {
            let _ = tx.send((
                server.cookie,
                server.default_sink_name.as_deref().map(str::to_owned),
                server.default_source_name.as_deref().map(str::to_owned),
            ));
        });
    connection.mainloop.unlock();
    let (cookie, output, input) = receive(rx)?;
    let (tx, rx) = mpsc::channel();
    let mut devices = Some(Vec::new());
    connection.mainloop.lock();
    connection
        .context
        .borrow()
        .introspect()
        .get_sink_info_list(move |item| {
            collect(&mut devices, &tx, item, |sink| {
                device(
                    DeviceKind::Output,
                    cookie,
                    session,
                    sink.index,
                    sink.name.as_deref(),
                    sink.description.as_deref(),
                    sink.volume,
                    sink.mute,
                    output.as_deref(),
                )
            });
        });
    connection.mainloop.unlock();
    let mut devices = receive(rx)??;
    let (tx, rx) = mpsc::channel();
    let mut inputs = Some(Vec::new());
    connection.mainloop.lock();
    connection
        .context
        .borrow()
        .introspect()
        .get_source_info_list(move |item| {
            collect(&mut inputs, &tx, item, |source| {
                if source.monitor_of_sink.is_some() {
                    return None;
                }
                device(
                    DeviceKind::Input,
                    cookie,
                    session,
                    source.index,
                    source.name.as_deref(),
                    source.description.as_deref(),
                    source.volume,
                    source.mute,
                    input.as_deref(),
                )
            });
        });
    connection.mainloop.unlock();
    devices.extend(receive(rx)??);
    devices.sort_by(|a, b| a.id.name.cmp(&b.id.name));
    Ok(AudioSnapshot { devices })
}

fn collect<T>(
    devices: &mut Option<Vec<AudioDevice>>,
    tx: &mpsc::Sender<Result<Vec<AudioDevice>, String>>,
    item: ListResult<&T>,
    convert: impl FnOnce(&T) -> Option<AudioDevice>,
) {
    let Some(current) = devices.as_mut() else {
        return;
    };
    match item {
        ListResult::Item(item) => {
            if current.len() >= MAX_DEVICES {
                *devices = None;
                let _ = tx.send(Err("Too many audio devices.".into()));
            } else if let Some(device) = convert(item) {
                current.push(device);
            }
        }
        ListResult::End => {
            let _ = tx.send(Ok(devices.take().unwrap()));
        }
        ListResult::Error => {
            *devices = None;
            let _ = tx.send(Err("Could not read audio devices.".into()));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn device(
    kind: DeviceKind,
    server_cookie: u32,
    session: u64,
    index: u32,
    name: Option<&str>,
    description: Option<&str>,
    channels: ChannelVolumes,
    muted: bool,
    default: Option<&str>,
) -> Option<AudioDevice> {
    let name = name.filter(|name| !name.is_empty() && name.len() <= 1024)?;
    Some(AudioDevice {
        id: AudioDeviceId {
            kind,
            server_cookie,
            session,
            index,
            name: name.into(),
        },
        description: description.unwrap_or(name).chars().take(256).collect(),
        volume: (f64::from(channels.avg().0) / f64::from(pulse::volume::Volume::NORMAL.0) * 100.0)
            .clamp(0.0, 150.0),
        muted,
        is_default: default == Some(name),
        channels,
    })
}

/// A request is valid only for this live connection and this exact device.
fn apply_to(
    connection: &mut PulseConnection,
    session: u64,
    id: &AudioDeviceId,
    operation: DeviceOperation,
) -> Result<(), String> {
    if id.session != session {
        return Err("The audio connection changed. Choose a current device.".into());
    }
    let current = snapshot(connection, session)?;
    let device = current
        .devices
        .iter()
        .find(|device| device.id == *id)
        .ok_or("The audio device is no longer available. Choose a current device.")?;
    // An external mixer may allow amplification. An increase must never turn
    // into a decrease when such a device is already above our normal-volume cap.
    if matches!(operation, DeviceOperation::AdjustVolume { up: true, .. })
        && !device.can_increase_volume()
    {
        return Ok(());
    }
    let (tx, rx) = mpsc::channel();
    let callback = move |success| {
        let _ = tx.send(success);
    };
    connection.mainloop.lock();
    {
        let mut context = connection.context.borrow_mut();
        match operation {
            DeviceOperation::SetDefault => match id.kind {
                DeviceKind::Output => {
                    context.set_default_sink(&id.name, callback);
                }
                DeviceKind::Input => {
                    context.set_default_source(&id.name, callback);
                }
            },
            DeviceOperation::SetMuted(muted) => match id.kind {
                DeviceKind::Output => {
                    context.introspect().set_sink_mute_by_index(
                        id.index,
                        muted,
                        Some(Box::new(callback)),
                    );
                }
                DeviceKind::Input => {
                    context.introspect().set_source_mute_by_index(
                        id.index,
                        muted,
                        Some(Box::new(callback)),
                    );
                }
            },
            DeviceOperation::AdjustVolume { step, up } => {
                use pulse::volume::Volume;
                let mut channels = device.channels;
                let delta = u32::from(step.min(100)) * Volume::NORMAL.0 / 100;
                let maximum = if up {
                    channels.max().0.saturating_add(delta).min(Volume::NORMAL.0)
                } else {
                    channels.max().0.saturating_sub(delta).min(Volume::NORMAL.0)
                };
                channels.scale(Volume(maximum));
                match id.kind {
                    DeviceKind::Output => {
                        context.introspect().set_sink_volume_by_index(
                            id.index,
                            &channels,
                            Some(Box::new(callback)),
                        );
                    }
                    DeviceKind::Input => {
                        context.introspect().set_source_volume_by_index(
                            id.index,
                            &channels,
                            Some(Box::new(callback)),
                        );
                    }
                }
            }
        }
    }
    connection.mainloop.unlock();
    if receive(rx)? {
        Ok(())
    } else {
        Err("The audio server rejected the change.".into())
    }
}
