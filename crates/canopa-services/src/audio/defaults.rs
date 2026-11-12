//! The default output and input, for keys and on-screen indicators.
//!
//! [`watch_defaults`] follows only the server's default sink and source,
//! event-driven: its thread parks between server events and opens no stream.
//! [`change_default`] adjusts or mutes whichever device is the default when
//! the request runs, serialized so rapid key repeats never lose a step.
use super::{AudioError, DeviceKind, PulseConnection, pulse, volume};
use crate::ServiceState;
use pulse::callbacks::ListResult;
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    task::{Context, Wake, Waker},
    time::Duration,
};
use tokio::sync::watch;

const DEADLINE: Duration = Duration::from_secs(2);

/// One default device as a person sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultDevice {
    pub kind: DeviceKind,
    /// The server's device name, stable while the device exists.
    pub name: String,
    /// The human-readable description, bounded to 256 characters.
    pub description: String,
    /// The loudest channel in percent; above 100 only when another program
    /// amplified it. Keys never raise it past 100.
    pub percent: u16,
    pub muted: bool,
}

/// The current default output and input. `input` is `None` when the default
/// source is a monitor of an output (no microphone).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Defaults {
    pub output: Option<DefaultDevice>,
    pub input: Option<DefaultDevice>,
}

/// A change to the default device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefaultChange {
    /// Raise the loudest channel by this many percent, at most to 100, and
    /// unmute: a person pressing "louder" wants to hear something.
    Raise(u8),
    /// Lower the loudest channel; mute is unchanged.
    Lower(u8),
    /// Set the loudest channel to this percent (0-100), keeping the balance
    /// and the mute state.
    Set(u8),
    ToggleMute,
}

struct Unpark(std::thread::Thread);

impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// Follow the default output and input of `server` (`None`: the session's
/// server). The first value is the current state; changes follow as the
/// server reports them. Dropping the receiver stops the observer thread.
/// A lost server is `Unavailable` once, then retried with bounded backoff.
pub fn watch_defaults(server: Option<String>) -> watch::Receiver<ServiceState<Defaults>> {
    let (tx, rx) = watch::channel(ServiceState::Unavailable);
    std::thread::Builder::new()
        .name("canopa-audio-defaults".into())
        .spawn(move || {
            let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
            let mut task = Context::from_waker(&waker);
            let mut closed = std::pin::pin!(tx.closed());
            let mut backoff = Duration::from_millis(500);
            loop {
                if closed.as_mut().poll(&mut task).is_ready() {
                    return;
                }
                if observe(server.as_deref(), &tx, &mut closed, &mut task) {
                    backoff = Duration::from_millis(500);
                }
                tx.send_if_modified(|state| {
                    std::mem::replace(state, ServiceState::Unavailable).is_available()
                });
                // Parked waits end early when the receiver goes away.
                let until = std::time::Instant::now() + backoff;
                while let Some(left) = until.checked_duration_since(std::time::Instant::now()) {
                    if closed.as_mut().poll(&mut task).is_ready() {
                        return;
                    }
                    std::thread::park_timeout(left);
                }
                backoff = (backoff * 2).min(Duration::from_secs(30));
            }
        })
        .expect("spawn the audio defaults observer");
    rx
}

/// Observe one connection until it fails or the receiver closes. Returns
/// whether the connection was ready at least once.
fn observe(
    server: Option<&str>,
    tx: &watch::Sender<ServiceState<Defaults>>,
    closed: &mut std::pin::Pin<&mut impl Future<Output = ()>>,
    task: &mut Context<'_>,
) -> bool {
    use pulse::context::subscribe::{Facility, InterestMaskSet};
    let Ok(mut connection) = PulseConnection::connect(server, || tx.is_closed()) else {
        return false;
    };
    let thread = std::thread::current();
    let dirty = Arc::new(AtomicBool::new(true));
    connection.mainloop.lock();
    let state_thread = thread.clone();
    connection
        .context
        .borrow_mut()
        .set_state_callback(Some(Box::new(move || state_thread.unpark())));
    let (changed, event_thread) = (dirty.clone(), thread.clone());
    connection
        .context
        .borrow_mut()
        .set_subscribe_callback(Some(Box::new(move |facility, _, _| {
            if matches!(
                facility,
                Some(Facility::Sink | Facility::Source | Facility::Server)
            ) {
                changed.store(true, Ordering::Release);
                event_thread.unpark();
            }
        })));
    let (ready_tx, ready_rx) = mpsc::channel();
    connection.context.borrow_mut().subscribe(
        InterestMaskSet::SINK | InterestMaskSet::SOURCE | InterestMaskSet::SERVER,
        move |success| {
            let _ = ready_tx.send(success);
        },
    );
    connection.mainloop.unlock();
    if ready_rx.recv_timeout(DEADLINE) != Ok(true) {
        return false;
    }
    loop {
        if closed.as_mut().poll(task).is_ready() {
            return true;
        }
        if dirty.swap(false, Ordering::AcqRel) {
            let Ok(defaults) = read(&mut connection) else {
                return true;
            };
            // A newer server event supersedes this reading; read again first.
            if !dirty.load(Ordering::Acquire) {
                tx.send_if_modified(|state| {
                    let next = ServiceState::Available(defaults);
                    if *state == next {
                        false
                    } else {
                        *state = next;
                        true
                    }
                });
            }
            continue;
        }
        connection.mainloop.lock();
        let state = connection.context.borrow().get_state();
        connection.mainloop.unlock();
        if state != pulse::context::State::Ready {
            return true;
        }
        // Server events, state changes and receiver cancellation unpark this
        // thread; park keeps a wake that arrives between the checks and here.
        std::thread::park();
    }
}

fn read(connection: &mut PulseConnection) -> Result<Defaults, AudioError> {
    Ok(Defaults {
        output: device(connection, DeviceKind::Output)?,
        input: device(connection, DeviceKind::Input)?,
    })
}

type Reading = (DefaultDevice, u32, pulse::volume::ChannelVolumes);

/// The current default device of `kind`, with its index and channels.
fn current(
    connection: &mut PulseConnection,
    kind: DeviceKind,
) -> Result<Option<Reading>, AudioError> {
    let (tx, rx) = mpsc::channel();
    connection.mainloop.lock();
    let introspect = connection.context.borrow().introspect();
    match kind {
        DeviceKind::Output => {
            let mut sent = false;
            introspect.get_sink_info_by_name("@DEFAULT_SINK@", move |result| {
                if sent {
                    return;
                }
                sent = true;
                let _ = tx.send(match result {
                    ListResult::Item(sink) => reading(
                        kind,
                        sink.index,
                        sink.name.as_deref(),
                        sink.description.as_deref(),
                        sink.volume,
                        sink.mute,
                    ),
                    _ => None,
                });
            });
        }
        DeviceKind::Input => {
            let mut sent = false;
            introspect.get_source_info_by_name("@DEFAULT_SOURCE@", move |result| {
                if sent {
                    return;
                }
                sent = true;
                let _ = tx.send(match result {
                    ListResult::Item(source) if source.monitor_of_sink.is_none() => reading(
                        kind,
                        source.index,
                        source.name.as_deref(),
                        source.description.as_deref(),
                        source.volume,
                        source.mute,
                    ),
                    _ => None,
                });
            });
        }
    }
    connection.mainloop.unlock();
    rx.recv_timeout(DEADLINE)
        .map_err(|_| AudioError::NoResponse)
}

fn device(
    connection: &mut PulseConnection,
    kind: DeviceKind,
) -> Result<Option<DefaultDevice>, AudioError> {
    Ok(current(connection, kind)?.map(|(device, _, _)| device))
}

fn percent(channels: &pulse::volume::ChannelVolumes) -> u16 {
    let normal = u64::from(pulse::volume::Volume::NORMAL.0);
    ((u64::from(channels.max().0) * 100 + normal / 2) / normal).min(999) as u16
}

fn reading(
    kind: DeviceKind,
    index: u32,
    name: Option<&str>,
    description: Option<&str>,
    channels: pulse::volume::ChannelVolumes,
    muted: bool,
) -> Option<Reading> {
    let name = name.filter(|name| !name.is_empty() && name.len() <= 1024)?;
    if !channels.is_valid() {
        return None;
    }
    Some((
        DefaultDevice {
            kind,
            name: name.into(),
            description: description
                .filter(|text| !text.trim().is_empty())
                .unwrap_or(name)
                .chars()
                .filter(|c| !c.is_control())
                .take(256)
                .collect(),
            percent: percent(&channels),
            muted,
        },
        index,
        channels,
    ))
}

/// Apply `change` to the current default device of `kind` on `server` and
/// return the device as it is afterwards. Requests are serialized.
pub async fn change_default(
    server: Option<String>,
    kind: DeviceKind,
    change: DefaultChange,
) -> Result<DefaultDevice, AudioError> {
    static CHANGES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = CHANGES.lock().await;
    tokio::task::spawn_blocking(move || change_on(server.as_deref(), kind, change))
        .await
        .map_err(|error| AudioError::Connection(error.to_string()))?
}

fn change_on(
    server: Option<&str>,
    kind: DeviceKind,
    change: DefaultChange,
) -> Result<DefaultDevice, AudioError> {
    let mut connection = PulseConnection::connect(server, || false)?;
    let (mut device, index, channels) =
        current(&mut connection, kind)?.ok_or(AudioError::NoDefaultSink)?;
    let (volume, muted) = match change {
        DefaultChange::Raise(step) => (
            Some(volume::apply(
                channels,
                volume::Change::Adjust { step, up: true },
            )?),
            false,
        ),
        DefaultChange::Lower(step) => (
            Some(volume::apply(
                channels,
                volume::Change::Adjust { step, up: false },
            )?),
            device.muted,
        ),
        DefaultChange::Set(percent) => (
            Some(volume::apply(channels, volume::Change::Set { percent })?),
            device.muted,
        ),
        DefaultChange::ToggleMute => (None, !device.muted),
    };
    let (tx, rx) = mpsc::channel();
    let mut expected = 0;
    connection.mainloop.lock();
    {
        let context = connection.context.borrow();
        let mut introspect = context.introspect();
        if let Some(volume) = volume.filter(|volume| *volume != channels) {
            expected += 1;
            let tx = tx.clone();
            let done = Some(Box::new(move |success| {
                let _ = tx.send(success);
            }) as Box<dyn FnMut(bool)>);
            match kind {
                DeviceKind::Output => {
                    introspect.set_sink_volume_by_index(index, &volume, done);
                }
                DeviceKind::Input => {
                    introspect.set_source_volume_by_index(index, &volume, done);
                }
            }
            device.percent = percent(&volume);
        }
        if muted != device.muted {
            expected += 1;
            let tx = tx.clone();
            let done = Some(Box::new(move |success| {
                let _ = tx.send(success);
            }) as Box<dyn FnMut(bool)>);
            match kind {
                DeviceKind::Output => {
                    introspect.set_sink_mute_by_index(index, muted, done);
                }
                DeviceKind::Input => {
                    introspect.set_source_mute_by_index(index, muted, done);
                }
            }
            device.muted = muted;
        }
    }
    connection.mainloop.unlock();
    for _ in 0..expected {
        if !rx
            .recv_timeout(DEADLINE)
            .map_err(|_| AudioError::AdjustmentTimedOut)?
        {
            return Err(AudioError::AdjustmentRejected);
        }
    }
    Ok(device)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::audio::AudioServer;

    async fn defaults_matching(
        state: &mut watch::Receiver<ServiceState<Defaults>>,
        matches: impl Fn(&Defaults) -> bool,
    ) -> Defaults {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let ServiceState::Available(defaults) = state.borrow_and_update().clone()
                    && matches(&defaults)
                {
                    return defaults;
                }
                state.changed().await.unwrap();
            }
        })
        .await
        .expect("audio defaults did not reach the expected state")
    }

    async fn microphone(server: &AudioServer) {
        server
            .pactl(&[
                "load-module",
                "module-null-sink",
                "sink_name=microphone",
                "media.class=Audio/Source/Virtual",
            ])
            .await;
        server.pactl(&["set-default-source", "microphone"]).await;
        server
            .run(
                "pw-metadata",
                &[
                    "-n",
                    "default",
                    "0",
                    "default.audio.source",
                    r#"{"name":"microphone"}"#,
                    "Spa:String:JSON",
                ],
            )
            .await;
    }

    #[tokio::test]
    async fn keys_clamp_raise_unmutes_and_the_watch_follows_outside_changes() {
        let server = AudioServer::new().await;
        let mut state = watch_defaults(Some(server.server.clone()));
        let empty = defaults_matching(&mut state, |d| d.output.is_none()).await;
        assert_eq!(empty.input, None, "no microphone without a source");
        assert_eq!(
            change_default(
                Some(server.server.clone()),
                DeviceKind::Output,
                DefaultChange::Raise(5)
            )
            .await,
            Err(AudioError::NoDefaultSink)
        );
        server
            .pactl(&["load-module", "module-null-sink", "sink_name=speakers"])
            .await;
        server.set_default_sink("speakers").await;
        server.pactl(&["set-sink-volume", "speakers", "97%"]).await;
        server.pactl(&["set-sink-mute", "speakers", "1"]).await;
        let seen = defaults_matching(&mut state, |d| {
            d.output
                .as_ref()
                .is_some_and(|o| o.percent == 97 && o.muted)
        })
        .await;
        assert_eq!(seen.output.unwrap().name, "speakers");

        let change =
            |change| change_default(Some(server.server.clone()), DeviceKind::Output, change);
        let raised = change(DefaultChange::Raise(5)).await.unwrap();
        assert_eq!((raised.percent, raised.muted), (100, false));
        assert_eq!(change(DefaultChange::Raise(5)).await.unwrap().percent, 100);
        let lowered = change(DefaultChange::Lower(30)).await.unwrap();
        assert_eq!((lowered.percent, lowered.muted), (70, false));
        assert!(change(DefaultChange::ToggleMute).await.unwrap().muted);
        assert_eq!(change(DefaultChange::Lower(100)).await.unwrap().percent, 0);
        assert!(change(DefaultChange::Lower(5)).await.unwrap().muted);
        assert!(
            server
                .pactl(&["get-sink-mute", "speakers"])
                .await
                .contains("yes")
        );
        defaults_matching(&mut state, |d| {
            d.output.as_ref().is_some_and(|o| o.percent == 0 && o.muted)
        })
        .await;
        // Another program amplified the output: a raise keeps the level.
        server.pactl(&["set-sink-volume", "speakers", "120%"]).await;
        assert_eq!(change(DefaultChange::Raise(5)).await.unwrap().percent, 120);

        microphone(&server).await;
        let input = defaults_matching(&mut state, |d| d.input.is_some()).await;
        assert_eq!(input.input.as_ref().unwrap().name, "microphone");
        let toggled = change_default(
            Some(server.server.clone()),
            DeviceKind::Input,
            DefaultChange::ToggleMute,
        )
        .await
        .unwrap();
        assert!(toggled.muted);
        defaults_matching(&mut state, |d| d.input.as_ref().is_some_and(|i| i.muted)).await;
    }

    #[tokio::test]
    async fn a_lost_server_is_unavailable_and_recovers() {
        let mut server = AudioServer::new().await;
        server
            .pactl(&["load-module", "module-null-sink", "sink_name=speakers"])
            .await;
        server.set_default_sink("speakers").await;
        let mut state = watch_defaults(Some(server.server.clone()));
        defaults_matching(&mut state, |d| d.output.is_some()).await;
        server.stop_pulse();
        tokio::time::timeout(Duration::from_secs(5), async {
            while state.borrow_and_update().is_available() {
                state.changed().await.unwrap();
            }
        })
        .await
        .expect("a lost server is unavailable");
        assert!(
            change_default(
                Some(server.server.clone()),
                DeviceKind::Output,
                DefaultChange::ToggleMute
            )
            .await
            .is_err()
        );
        server.restart_pulse();
        // The fixture's pipewire-pulse has no persistent sink; recovery is
        // the return to an available state.
        tokio::time::timeout(Duration::from_secs(10), async {
            while !state.borrow_and_update().is_available() {
                state.changed().await.unwrap();
            }
        })
        .await
        .expect("the watch reconnects");
    }
}
