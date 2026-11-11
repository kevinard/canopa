//! Audio devices and the default output through the PulseAudio protocol,
//! which PipeWire's compatibility server also implements. Behind the `audio`
//! feature, because it links libpulse.
//!
//! - [`monitor`] keeps the latest snapshot of outputs and inputs (no monitor
//!   sources) and serializes identity-bound [`DeviceOperation`]s. A device id
//!   is valid only for the connection that listed it; a reconnect or a
//!   removed device is [`AudioError::ConnectionChanged`] or
//!   [`AudioError::DeviceGone`], never another device.
//! - [`run_pa_thread`] and [`adjust_default_sink_volume`] are the bar's
//!   lightweight default-output observer and scroll control.
//! - [`watch_defaults`] and [`change_default`] follow and change the default
//!   output and input for device keys and on-screen indicators.
//!
//! Volume changes keep channel proportions, cap the loudest channel at 100 %
//! and leave mute unchanged. Discovery opens no capture stream.
//!
//! ```no_run
//! use canopa_services::audio::{self, DeviceKind, DeviceOperation};
//! # async fn page() {
//! audio::set_client_name("canopa-settings");
//! let (control, mut state) = audio::monitor(None); // `None`: the default server.
//! while state.changed().await.is_ok() {
//!     let Some(snapshot) = state.borrow_and_update().clone() else {
//!         println!("No audio server"); // Unavailable; the monitor reconnects.
//!         continue;
//!     };
//!     for device in snapshot.devices.iter().filter(|d| d.id.kind == DeviceKind::Output) {
//!         println!("{} {:.0} %", device.description, device.volume);
//!     }
//!     if let Some(output) = snapshot.devices.first() {
//!         let result = control
//!             .apply(output.id.clone(), DeviceOperation::SetVolume { percent: 40 })
//!             .await;
//!         if let Err(error) = result {
//!             eprintln!("{error}");
//!         }
//!     }
//! }
//! # }
//! ```
use tokio::sync::mpsc;

mod defaults;
mod devices;
mod volume;
pub use defaults::{DefaultChange, DefaultDevice, Defaults, change_default, watch_defaults};
pub use devices::{
    AudioControl, AudioDevice, AudioDeviceId, AudioSnapshot, DeviceKind, DeviceOperation, monitor,
};

/// Why an audio read or command failed. The messages are plain sentences
/// that panels show as they are.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AudioError {
    /// The command queue is full or the observer has stopped.
    #[error("Audio controls are busy or unavailable.")]
    Busy,
    #[error("Audio operation timed out.")]
    TimedOut,
    /// The observer ended before answering.
    #[error("Audio controls are unavailable.")]
    ControlsUnavailable,
    /// No audio server connection.
    #[error("Audio is unavailable.")]
    Unavailable,
    #[error("Audio server did not respond.")]
    NoResponse,
    #[error("Too many audio devices.")]
    TooManyDevices,
    #[error("Could not read audio devices.")]
    ReadFailed,
    /// The id came from an earlier connection.
    #[error("The audio connection changed. Choose a current device.")]
    ConnectionChanged,
    #[error("The audio device is no longer available. Choose a current device.")]
    DeviceGone,
    #[error("Invalid audio channel volumes.")]
    InvalidChannels,
    #[error("Volume must be between 0% and 100%.")]
    VolumeOutOfRange,
    #[error("The audio server rejected the change.")]
    Rejected,
    #[error("default audio sink is unavailable")]
    NoDefaultSink,
    #[error("audio server rejected volume adjustment")]
    AdjustmentRejected,
    #[error("audio adjustment timed out")]
    AdjustmentTimedOut,
    /// Connecting to the server failed; the detail is for logs.
    #[error("{0}")]
    Connection(String),
}

static CLIENT_NAME: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

/// The application name the audio server shows for this process's
/// connections (for example in a mixer). Set it once, before connecting;
/// the default is `canopa`.
pub fn set_client_name(name: &'static str) {
    let _ = CLIENT_NAME.set(name);
}

fn client_name() -> &'static str {
    CLIENT_NAME.get().copied().unwrap_or("canopa")
}

#[cfg(test)]
mod tests;

// Both observation and control use the same PulseAudio protocol/server selection.
// PipeWire's PulseAudio compatibility server implements that same contract.
use libpulse_binding as pulse;
use std::{
    cell::RefCell,
    future::Future,
    rc::Rc,
    sync::Arc,
    task::{Context, Wake, Waker},
    time::{Duration, Instant},
};
type VolumeSender = mpsc::UnboundedSender<Result<Option<(f64, bool)>, ()>>;

struct PulseConnection {
    context: Rc<RefCell<pulse::context::Context>>,
    mainloop: pulse::mainloop::threaded::Mainloop,
}

impl PulseConnection {
    fn connect(server: Option<&str>, cancelled: impl Fn() -> bool) -> Result<Self, AudioError> {
        let mut mainloop = pulse::mainloop::threaded::Mainloop::new()
            .ok_or_else(|| AudioError::Connection("cannot create audio mainloop".into()))?;
        let context = pulse::context::Context::new(&mainloop, client_name())
            .ok_or_else(|| AudioError::Connection("cannot create audio context".into()))?;
        mainloop
            .start()
            .map_err(|error| AudioError::Connection(format!("audio mainloop: {error:?}")))?;
        let mut connection = Self {
            context: Rc::new(RefCell::new(context)),
            mainloop,
        };
        connection.mainloop.lock();
        let result = connection.context.borrow_mut().connect(
            server,
            pulse::context::FlagSet::NOAUTOSPAWN,
            None,
        );
        connection.mainloop.unlock();
        result.map_err(|error| AudioError::Connection(format!("audio connection: {error:?}")))?;
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            connection.mainloop.lock();
            let state = connection.context.borrow().get_state();
            connection.mainloop.unlock();
            match state {
                pulse::context::State::Ready => return Ok(connection),
                pulse::context::State::Failed | pulse::context::State::Terminated => {
                    return Err(AudioError::Connection("audio server unavailable".into()));
                }
                _ if cancelled() || Instant::now() >= deadline => {
                    return Err(AudioError::Connection(
                        "audio connection cancelled or timed out".into(),
                    ));
                }
                _ => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    }
}

impl Drop for PulseConnection {
    fn drop(&mut self) {
        self.mainloop.lock();
        self.context.borrow_mut().set_subscribe_callback(None);
        self.context.borrow_mut().set_state_callback(None);
        self.context.borrow_mut().disconnect();
        self.mainloop.unlock();
        self.mainloop.stop();
    }
}

/// The bar's default-output observer: sends `Ok(Some((percent, muted)))` on
/// each change, `Ok(None)` without a default sink, and `Err(())` once when the
/// connection fails. Runs on the calling thread until `tx` closes.
pub fn run_pa_thread(tx: VolumeSender) {
    run_pa_thread_on(None, tx);
}

struct VolumeWake(std::thread::Thread);

impl Wake for VolumeWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn run_pa_thread_on(server: Option<&str>, tx: VolumeSender) {
    use pulse::context::subscribe::{Facility, InterestMaskSet};
    let Ok(mut connection) = PulseConnection::connect(server, || tx.is_closed()) else {
        let _ = tx.send(Err(()));
        return;
    };
    connection.mainloop.lock();
    let thread = std::thread::current();
    let state_thread = thread.clone();
    connection
        .context
        .borrow_mut()
        .set_state_callback(Some(Box::new(move || state_thread.unpark())));
    let revision = Rc::new(std::cell::Cell::new(0));
    let tx_cb = tx.clone();
    let context_cb = Rc::clone(&connection.context);
    let revision_cb = Rc::clone(&revision);
    connection
        .context
        .borrow_mut()
        .set_subscribe_callback(Some(Box::new(move |facility, _, _| {
            // Removed is as important as Changed: losing the last sink must clear state.
            if matches!(facility, Some(Facility::Sink | Facility::Server)) {
                fetch_volume(&context_cb.borrow(), &tx_cb, &revision_cb);
            }
        })));
    let subscription_tx = tx.clone();
    connection.context.borrow_mut().subscribe(
        InterestMaskSet::SINK | InterestMaskSet::SERVER,
        move |success| {
            if !success {
                let _ = subscription_tx.send(Err(()));
            }
        },
    );
    fetch_volume(&connection.context.borrow(), &tx, &revision);
    connection.mainloop.unlock();
    let waker = Waker::from(Arc::new(VolumeWake(thread)));
    let mut task = Context::from_waker(&waker);
    let mut closed = std::pin::pin!(tx.closed());
    loop {
        if closed.as_mut().poll(&mut task).is_ready() {
            return;
        }
        connection.mainloop.lock();
        let state = connection.context.borrow().get_state();
        connection.mainloop.unlock();
        if matches!(
            state,
            pulse::context::State::Failed | pulse::context::State::Terminated
        ) {
            break;
        }
        // Both receiver cancellation and Pulse state changes unpark this thread.
        // park preserves a wake arriving between the checks and the wait.
        std::thread::park();
    }
    let _ = tx.send(Err(()));
}

fn fetch_volume(
    ctx: &pulse::context::Context,
    tx: &VolumeSender,
    revision: &Rc<std::cell::Cell<u64>>,
) {
    let current = revision.get().wrapping_add(1);
    revision.set(current);
    let revision = Rc::clone(revision);
    let tx = tx.clone();
    let mut found = false;
    ctx.introspect()
        .get_sink_info_by_name("@DEFAULT_SINK@", move |result| {
            use pulse::callbacks::ListResult;
            if revision.get() != current {
                return;
            }
            match result {
                ListResult::Item(sink) => {
                    found = true;
                    let pct = (sink.volume.avg().0 as f64 / pulse::volume::Volume::NORMAL.0 as f64
                        * 100.0)
                        .clamp(0.0, 150.0);
                    let _ = tx.send(Ok(Some((pct, sink.mute))));
                }
                ListResult::Error => {
                    let _ = tx.send(Ok(None));
                }
                ListResult::End if !found => {
                    let _ = tx.send(Ok(None));
                }
                ListResult::End => {}
            }
        });
}

/// Raise or lower the default output by `step` percent, serialized so rapid
/// scroll events do not lose increments.
pub async fn adjust_default_sink_volume(step: u8, up: bool) -> Result<(), AudioError> {
    // Serialize read/modify/write so rapid scroll events do not lose increments.
    static ADJUSTMENT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = ADJUSTMENT.lock().await;
    tokio::task::spawn_blocking(move || adjust_volume_on(None, step, up))
        .await
        .map_err(|error| AudioError::Connection(error.to_string()))?
}

fn adjust_volume_on(server: Option<&str>, step: u8, up: bool) -> Result<(), AudioError> {
    use pulse::callbacks::ListResult;
    let mut connection = PulseConnection::connect(server, || false)?;
    let (tx, rx) = std::sync::mpsc::channel();
    let context = Rc::clone(&connection.context);
    let mut found = false;
    connection.mainloop.lock();
    connection
        .context
        .borrow()
        .introspect()
        .get_sink_info_by_name("@DEFAULT_SINK@", move |result| match result {
            ListResult::Item(sink) => {
                found = true;
                let volume = match volume::apply(sink.volume, volume::Change::Adjust { step, up }) {
                    Ok(volume) => volume,
                    Err(error) => {
                        let _ = tx.send(Err(error));
                        return;
                    }
                };
                if volume == sink.volume {
                    let _ = tx.send(Ok(()));
                    return;
                }
                let tx = tx.clone();
                context.borrow().introspect().set_sink_volume_by_index(
                    sink.index,
                    &volume,
                    Some(Box::new(move |success| {
                        let _ = tx.send(if success {
                            Ok(())
                        } else {
                            Err(AudioError::AdjustmentRejected)
                        });
                    })),
                );
            }
            ListResult::Error => {
                let _ = tx.send(Err(AudioError::NoDefaultSink));
            }
            ListResult::End if !found => {
                let _ = tx.send(Err(AudioError::NoDefaultSink));
            }
            ListResult::End => {}
        });
    connection.mainloop.unlock();
    rx.recv_timeout(Duration::from_secs(2))
        .map_err(|_| AudioError::AdjustmentTimedOut)?
}
