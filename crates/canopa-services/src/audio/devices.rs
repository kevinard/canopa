//! Bounded Pulse-protocol device queries and explicit, identity-bound operations.
use super::{AudioError, PulseConnection, pulse};
use pulse::{callbacks::ListResult, channelmap::Map, volume::ChannelVolumes};
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
    /// Left–right balance, -100 (left) to 100 (right), when the channel
    /// layout has a left and a right side.
    pub balance: Option<i8>,
    channels: ChannelVolumes,
    map: Map,
}

impl AudioDevice {
    pub fn max_volume_percent(&self) -> f64 {
        f64::from(self.channels.max().0) / f64::from(pulse::volume::Volume::NORMAL.0) * 100.0
    }
    pub fn can_increase_volume(&self) -> bool {
        self.channels.max() < pulse::volume::Volume::NORMAL
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioSnapshot {
    pub devices: Vec<AudioDevice>,
    /// Applications playing or recording, from [`mixer`] only.
    pub streams: Vec<AudioStream>,
    /// Sound cards and their profiles, from [`mixer`] only.
    pub cards: Vec<AudioCard>,
}

/// Whether an application plays (a sink input) or records (a source output).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamKind {
    Playback,
    Recording,
}

/// An application stream, valid for the connection that listed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioStreamId {
    pub kind: StreamKind,
    pub index: u32,
    pub server_cookie: u32,
    pub session: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioStream {
    pub id: AudioStreamId,
    /// The application's name, or its program when it gives none.
    pub application: String,
    /// What it plays or records, such as a song or a call.
    pub title: String,
    pub icon: Option<String>,
    pub volume: f64,
    pub muted: bool,
    /// Whether the server lets clients change its volume.
    pub volume_writable: bool,
    /// The output or input device index it uses, once it is linked.
    pub device: Option<u32>,
    channels: ChannelVolumes,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardProfile {
    pub name: String,
    pub description: String,
    /// False when the profile cannot be used now (for example HDMI without a
    /// connected display).
    pub available: bool,
}

/// A sound card with its profiles, such as HDMI or analog output, or a
/// Bluetooth headset's high-fidelity (A2DP) and headset (HFP) profiles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioCard {
    pub index: u32,
    pub name: String,
    pub description: String,
    pub profiles: Vec<CardProfile>,
    pub active: Option<String>,
    pub server_cookie: u32,
    pub session: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceOperation {
    AdjustVolume {
        step: u8,
        up: bool,
    },
    SetVolume {
        percent: u8,
    },
    SetMuted(bool),
    SetDefault,
    /// -100 (left) to 100 (right); the loudest channel keeps its level.
    SetBalance {
        balance: i8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamOperation {
    SetVolume { percent: u8 },
    SetMuted(bool),
}

/// What a queued request changes.
enum Target {
    Device(AudioDeviceId, DeviceOperation),
    Stream(AudioStreamId, StreamOperation),
    Profile {
        card: u32,
        session: u64,
        name: String,
        profile: String,
    },
}

#[derive(Clone)]
pub struct AudioControl {
    commands: tokio::sync::mpsc::Sender<OperationRequest>,
}
struct OperationRequest {
    target: Target,
    reply: tokio::sync::oneshot::Sender<Result<(), AudioError>>,
}
impl AudioControl {
    pub async fn apply(
        &self,
        id: AudioDeviceId,
        operation: DeviceOperation,
    ) -> Result<(), AudioError> {
        self.request(Target::Device(id, operation)).await
    }

    /// Change one application's stream.
    pub async fn apply_stream(
        &self,
        id: AudioStreamId,
        operation: StreamOperation,
    ) -> Result<(), AudioError> {
        self.request(Target::Stream(id, operation)).await
    }

    /// Switch a card to one of its profiles.
    pub async fn set_profile(&self, card: &AudioCard, profile: &str) -> Result<(), AudioError> {
        self.request(Target::Profile {
            card: card.index,
            session: card.session,
            name: card.name.clone(),
            profile: profile.into(),
        })
        .await
    }

    async fn request(&self, target: Target) -> Result<(), AudioError> {
        let (reply, response) = tokio::sync::oneshot::channel();
        self.commands
            .try_send(OperationRequest { target, reply })
            .map_err(|_| AudioError::Busy)?;
        tokio::time::timeout(Duration::from_secs(10), response)
            .await
            .map_err(|_| AudioError::TimedOut)?
            .map_err(|_| AudioError::ControlsUnavailable)?
    }
}

/// Only the most recent snapshot is retained. Dropping the receiver stops the
/// observer and its command worker, including reconnects. No audio is recorded.
pub fn monitor(server: Option<String>) -> (AudioControl, watch::Receiver<Option<AudioSnapshot>>) {
    observer(server, false)
}

/// [`monitor`] that also lists application streams and cards, for a mixer.
pub fn mixer(server: Option<String>) -> (AudioControl, watch::Receiver<Option<AudioSnapshot>>) {
    observer(server, true)
}

fn observer(
    server: Option<String>,
    mixer: bool,
) -> (AudioControl, watch::Receiver<Option<AudioSnapshot>>) {
    let (tx, rx) = watch::channel(None);
    let (commands, mut requests) = tokio::sync::mpsc::channel::<OperationRequest>(8);
    std::thread::spawn(move || {
        while !tx.is_closed() {
            observe(server.as_deref(), mixer, &tx, &mut requests);
            tx.send_if_modified(|value| value.take().is_some());
            for _ in 0..20 {
                if tx.is_closed() {
                    return;
                }
                while let Ok(request) = requests.try_recv() {
                    let _ = request.reply.send(Err(AudioError::Unavailable));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    });
    (AudioControl { commands }, rx)
}

fn observe(
    server: Option<&str>,
    mixer: bool,
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
    let mut interest = InterestMaskSet::SINK | InterestMaskSet::SOURCE | InterestMaskSet::SERVER;
    if mixer {
        interest |=
            InterestMaskSet::SINK_INPUT | InterestMaskSet::SOURCE_OUTPUT | InterestMaskSet::CARD;
    }
    connection
        .context
        .borrow_mut()
        .subscribe(interest, move |success| {
            let _ = ready_tx.send(success);
        });
    connection.mainloop.unlock();
    if ready_rx.recv_timeout(DEADLINE) != Ok(true) {
        return;
    }
    while !tx.is_closed() {
        if let Ok(request) = requests.try_recv()
            && !request.reply.is_closed()
        {
            let result = match &request.target {
                Target::Device(id, operation) => apply_to(&mut connection, session, id, *operation),
                Target::Stream(id, operation) => {
                    apply_stream(&mut connection, session, id, *operation)
                }
                Target::Profile {
                    card,
                    session: card_session,
                    name,
                    profile,
                } => set_profile(
                    &mut connection,
                    session,
                    *card_session,
                    *card,
                    name,
                    profile,
                ),
            };
            let _ = request.reply.send(result);
            dirty.store(true, Ordering::Release);
        }
        if dirty.swap(false, Ordering::AcqRel) {
            let Ok(snapshot) = snapshot_with(&mut connection, session, mixer) else {
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

fn receive<T>(rx: mpsc::Receiver<T>) -> Result<T, AudioError> {
    rx.recv_timeout(DEADLINE)
        .map_err(|_| AudioError::NoResponse)
}

fn snapshot(connection: &mut PulseConnection, session: u64) -> Result<AudioSnapshot, AudioError> {
    snapshot_with(connection, session, false)
}

fn snapshot_with(
    connection: &mut PulseConnection,
    session: u64,
    mixer: bool,
) -> Result<AudioSnapshot, AudioError> {
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
                    sink.channel_map,
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
                    source.channel_map,
                    source.mute,
                    input.as_deref(),
                )
            });
        });
    connection.mainloop.unlock();
    devices.extend(receive(rx)??);
    devices.sort_by(|a, b| a.id.name.cmp(&b.id.name));
    if !mixer {
        return Ok(AudioSnapshot {
            devices,
            ..Default::default()
        });
    }
    let (tx, rx) = mpsc::channel();
    let mut streams = Some(Vec::new());
    connection.mainloop.lock();
    connection
        .context
        .borrow()
        .introspect()
        .get_sink_input_info_list(move |item| {
            collect(&mut streams, &tx, item, |input| {
                stream(
                    StreamKind::Playback,
                    input.index,
                    &input.proplist,
                    input.name.as_deref(),
                    input.volume,
                    input.mute,
                    input.has_volume && input.volume_writable,
                    input.sink,
                    cookie,
                    session,
                )
            });
        });
    connection.mainloop.unlock();
    let mut streams = receive(rx)??;
    let (tx, rx) = mpsc::channel();
    let mut recordings = Some(Vec::new());
    connection.mainloop.lock();
    connection
        .context
        .borrow()
        .introspect()
        .get_source_output_info_list(move |item| {
            collect(&mut recordings, &tx, item, |output| {
                // Peak meters such as this crate's own are not applications.
                let flags = output.proplist.get_str("application.id");
                if flags.as_deref() == Some(METER_APPLICATION) {
                    return None;
                }
                stream(
                    StreamKind::Recording,
                    output.index,
                    &output.proplist,
                    output.name.as_deref(),
                    output.volume,
                    output.mute,
                    output.has_volume && output.volume_writable,
                    output.source,
                    cookie,
                    session,
                )
            });
        });
    connection.mainloop.unlock();
    streams.extend(receive(rx)??);
    let (tx, rx) = mpsc::channel();
    let mut cards = Some(Vec::new());
    connection.mainloop.lock();
    connection
        .context
        .borrow()
        .introspect()
        .get_card_info_list(move |item| {
            collect(&mut cards, &tx, item, |card| {
                let name = card.name.as_deref().filter(|name| !name.is_empty())?;
                let description = card
                    .proplist
                    .get_str(pulse::proplist::properties::DEVICE_DESCRIPTION)
                    .unwrap_or_else(|| name.into());
                let mut profiles: Vec<_> = card
                    .profiles
                    .iter()
                    .take(64)
                    .filter_map(|profile| {
                        let name = profile.name.as_deref()?.to_owned();
                        Some((
                            profile.priority,
                            CardProfile {
                                description: profile
                                    .description
                                    .as_deref()
                                    .unwrap_or(&name)
                                    .chars()
                                    .take(128)
                                    .collect(),
                                name,
                                available: profile.available,
                            },
                        ))
                    })
                    .collect();
                // Highest priority first, as mixers list them.
                profiles.sort_by_key(|profile| std::cmp::Reverse(profile.0));
                Some(AudioCard {
                    index: card.index,
                    name: name.chars().take(1024).collect(),
                    description: description.chars().take(256).collect(),
                    profiles: profiles.into_iter().map(|(_, profile)| profile).collect(),
                    active: card
                        .active_profile
                        .as_ref()
                        .and_then(|profile| profile.name.as_deref().map(str::to_owned)),
                    server_cookie: cookie,
                    session,
                })
            });
        });
    connection.mainloop.unlock();
    let mut cards = receive(rx)??;
    cards.sort_by(|a, b| a.name.cmp(&b.name));
    streams.sort_by(|a, b| {
        (a.id.kind == StreamKind::Recording, a.id.index)
            .cmp(&(b.id.kind == StreamKind::Recording, b.id.index))
    });
    Ok(AudioSnapshot {
        devices,
        streams,
        cards,
    })
}

#[allow(clippy::too_many_arguments)]
fn stream(
    kind: StreamKind,
    index: u32,
    proplist: &pulse::proplist::Proplist,
    name: Option<&str>,
    volume: ChannelVolumes,
    muted: bool,
    writable: bool,
    device: u32,
    server_cookie: u32,
    session: u64,
) -> Option<AudioStream> {
    use pulse::proplist::properties;
    let text = |value: Option<String>| -> Option<String> {
        value
            .map(|value| value.trim().chars().take(128).collect::<String>())
            .filter(|value| !value.is_empty())
    };
    let application = text(proplist.get_str(properties::APPLICATION_NAME))
        .or_else(|| text(proplist.get_str(properties::APPLICATION_PROCESS_BINARY)))
        .unwrap_or_else(|| fl!("services-audio-unknown-application"));
    Some(AudioStream {
        id: AudioStreamId {
            kind,
            index,
            server_cookie,
            session,
        },
        title: text(proplist.get_str(properties::MEDIA_NAME))
            .or_else(|| text(name.map(str::to_owned)))
            .unwrap_or_default(),
        icon: text(proplist.get_str(properties::APPLICATION_ICON_NAME)),
        application,
        volume: percent(volume),
        muted,
        volume_writable: writable,
        device: (device != u32::MAX).then_some(device),
        channels: volume,
    })
}

/// The `application.id` of this crate's own level meter streams.
pub(super) const METER_APPLICATION: &str = "org.canopa.LevelMeter";

fn percent(channels: ChannelVolumes) -> f64 {
    (f64::from(channels.avg().0) / f64::from(pulse::volume::Volume::NORMAL.0) * 100.0)
        .clamp(0.0, 150.0)
}

fn collect<T, Item>(
    devices: &mut Option<Vec<Item>>,
    tx: &mpsc::Sender<Result<Vec<Item>, AudioError>>,
    item: ListResult<&T>,
    convert: impl FnOnce(&T) -> Option<Item>,
) {
    let Some(current) = devices.as_mut() else {
        return;
    };
    match item {
        ListResult::Item(item) => {
            if current.len() >= MAX_DEVICES {
                *devices = None;
                let _ = tx.send(Err(AudioError::TooManyDevices));
            } else if let Some(device) = convert(item) {
                current.push(device);
            }
        }
        ListResult::End => {
            let _ = tx.send(Ok(devices.take().unwrap()));
        }
        ListResult::Error => {
            *devices = None;
            let _ = tx.send(Err(AudioError::ReadFailed));
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
    map: Map,
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
        volume: percent(channels),
        muted,
        is_default: default == Some(name),
        balance: map.can_balance().then(|| {
            (channels.get_balance(&map) * 100.0)
                .round()
                .clamp(-100.0, 100.0) as i8
        }),
        channels,
        map,
    })
}

/// A request is valid only for this live connection and this exact device.
fn apply_to(
    connection: &mut PulseConnection,
    session: u64,
    id: &AudioDeviceId,
    operation: DeviceOperation,
) -> Result<(), AudioError> {
    if id.session != session {
        return Err(AudioError::ConnectionChanged);
    }
    let current = snapshot(connection, session)?;
    let device = current
        .devices
        .iter()
        .find(|device| device.id == *id)
        .ok_or(AudioError::DeviceGone)?;
    let channels = match operation {
        DeviceOperation::AdjustVolume { step, up } => Some(super::volume::apply(
            device.channels,
            super::volume::Change::Adjust { step, up },
        )?),
        DeviceOperation::SetVolume { percent } => Some(super::volume::apply(
            device.channels,
            super::volume::Change::Set { percent },
        )?),
        DeviceOperation::SetBalance { balance } => {
            if !device.map.can_balance() || !(-100..=100).contains(&balance) {
                return Err(AudioError::InvalidChannels);
            }
            let mut channels = device.channels;
            channels
                .set_balance(&device.map, f32::from(balance) / 100.0)
                .ok_or(AudioError::InvalidChannels)?;
            Some(channels)
        }
        _ => None,
    };
    if channels == Some(device.channels) {
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
            DeviceOperation::AdjustVolume { .. }
            | DeviceOperation::SetVolume { .. }
            | DeviceOperation::SetBalance { .. } => {
                let channels = channels.expect("volume operation has channel levels");
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
        Err(AudioError::Rejected)
    }
}

/// A stream request is valid only for this connection and a stream it still lists.
fn apply_stream(
    connection: &mut PulseConnection,
    session: u64,
    id: &AudioStreamId,
    operation: StreamOperation,
) -> Result<(), AudioError> {
    if id.session != session {
        return Err(AudioError::ConnectionChanged);
    }
    let current = snapshot_with(connection, session, true)?;
    let stream = current
        .streams
        .iter()
        .find(|stream| stream.id == *id)
        .ok_or(AudioError::DeviceGone)?;
    let (tx, rx) = mpsc::channel();
    let callback = move |success| {
        let _ = tx.send(success);
    };
    let volume = match operation {
        StreamOperation::SetVolume { percent } => {
            if !stream.volume_writable {
                return Err(AudioError::Rejected);
            }
            Some(super::volume::apply(
                stream.channels,
                super::volume::Change::Set { percent },
            )?)
        }
        StreamOperation::SetMuted(_) => None,
    };
    connection.mainloop.lock();
    {
        let mut introspect = connection.context.borrow().introspect();
        match (operation, id.kind, volume) {
            (StreamOperation::SetMuted(muted), StreamKind::Playback, _) => {
                introspect.set_sink_input_mute(id.index, muted, Some(Box::new(callback)));
            }
            (StreamOperation::SetMuted(muted), StreamKind::Recording, _) => {
                introspect.set_source_output_mute(id.index, muted, Some(Box::new(callback)));
            }
            (_, StreamKind::Playback, Some(volume)) => {
                introspect.set_sink_input_volume(id.index, &volume, Some(Box::new(callback)));
            }
            (_, StreamKind::Recording, Some(volume)) => {
                introspect.set_source_output_volume(id.index, &volume, Some(Box::new(callback)));
            }
            (StreamOperation::SetVolume { .. }, _, None) => unreachable!("volume computed above"),
        }
    }
    connection.mainloop.unlock();
    if receive(rx)? {
        Ok(())
    } else {
        Err(AudioError::Rejected)
    }
}

/// Switch a card that this connection still lists, by index and name, to a
/// profile it offers.
fn set_profile(
    connection: &mut PulseConnection,
    session: u64,
    card_session: u64,
    card: u32,
    name: &str,
    profile: &str,
) -> Result<(), AudioError> {
    if card_session != session {
        return Err(AudioError::ConnectionChanged);
    }
    let current = snapshot_with(connection, session, true)?;
    let found = current
        .cards
        .iter()
        .find(|candidate| candidate.index == card && candidate.name == name)
        .ok_or(AudioError::DeviceGone)?;
    if !found
        .profiles
        .iter()
        .any(|candidate| candidate.name == profile)
    {
        return Err(AudioError::Rejected);
    }
    let (tx, rx) = mpsc::channel();
    connection.mainloop.lock();
    connection
        .context
        .borrow()
        .introspect()
        .set_card_profile_by_index(
            card,
            profile,
            Some(Box::new(move |success| {
                let _ = tx.send(success);
            })),
        );
    connection.mainloop.unlock();
    if receive(rx)? {
        Ok(())
    } else {
        Err(AudioError::Rejected)
    }
}
