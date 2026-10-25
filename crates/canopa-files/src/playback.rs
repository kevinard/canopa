//! Lazy, renderer-independent playback of an already prepared local source.
//! One owner controls each session; dropping it cancels decoding without waiting.
mod decoder;
mod input;
#[cfg(test)]
mod tests;

use crate::{Cancellation, preview::MediaSource};
use futures_channel::mpsc as asynchronous;
use std::{
    io,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Preparing,
    Paused,
    Playing,
    Ended,
    Failed(String),
    Stopped,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub serial: u64,
    pub bitmap: crate::preview::Bitmap,
    pub timestamp: Option<Duration>,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub revision: u64,
    pub phase: Phase,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub seekable: bool,
    pub seeking: bool,
    pub volume: f64,
    pub muted: bool,
    pub frame: Option<Frame>,
    pub notice: Option<String>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            phase: Phase::Preparing,
            position: Duration::ZERO,
            duration: None,
            seekable: false,
            seeking: false,
            volume: 1.0,
            muted: false,
            frame: None,
            notice: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Command {
    Playing(bool),
    TogglePlayback,
    Seek(Duration),
    /// Seconds relative to the latest accepted seek or current decoder position.
    SeekBy(i64),
    Volume(f64),
    VolumeBy(f64),
    Muted(bool),
    ToggleMuted,
}

#[derive(Default)]
struct Activity {
    active: usize,
    closed: bool,
}

#[derive(Clone, Copy)]
enum AudioOutput {
    System,
    #[cfg(test)]
    Discard,
}

struct Runtime {
    activity: Mutex<Activity>,
    notice: Mutex<asynchronous::Sender<()>>,
    notices: Mutex<Option<asynchronous::Receiver<()>>>,
    audio: AudioOutput,
}

impl Runtime {
    fn notify(&self) {
        let _ = self
            .notice
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_send(());
    }
}

/// Creating or cloning this host does not initialize GStreamer or open a device.
#[derive(Clone)]
pub struct Playback(Arc<Runtime>);

impl Default for Playback {
    fn default() -> Self {
        Self::with_output(AudioOutput::System)
    }
}

impl Playback {
    /// Controller tests decode real sources without contacting an audio device.
    #[cfg(test)]
    pub(crate) fn silent() -> Self {
        Self::with_output(AudioOutput::Discard)
    }

    fn with_output(audio: AudioOutput) -> Self {
        let (notice, notices) = asynchronous::channel(0);
        Self(Arc::new(Runtime {
            activity: Mutex::new(Activity::default()),
            notice: Mutex::new(notice),
            notices: Mutex::new(Some(notices)),
            audio,
        }))
    }

    /// All reading, initialization and decoder transitions occur on the worker.
    /// Admission takes an activity hold before returning, including during startup.
    pub fn open(&self, source: MediaSource) -> io::Result<Session> {
        {
            let mut state = self.0.activity.lock().unwrap_or_else(|e| e.into_inner());
            if state.closed {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "Media playback has shut down.",
                ));
            }
            state.active += 1;
        }
        let activity = WorkerActivity(self.0.clone());
        let (commands, receiver) = mpsc::sync_channel(32);
        let shared = Arc::new(Shared {
            snapshot: Mutex::new(Snapshot::default()),
            cancellation: Cancellation::default(),
            runtime: self.0.clone(),
        });
        let worker = shared.clone();
        let audio = self.0.audio;
        std::thread::Builder::new()
            .name("files-media".into())
            .spawn(move || {
                let _activity = activity;
                decoder::run(source, worker, receiver, audio);
            })?;
        Ok(Session { shared, commands })
    }

    /// The host observes one coalesced wakeup and reads current session snapshots.
    /// Dropping observations never cancels a session or its cleanup.
    pub fn take_notices(&self) -> Option<asynchronous::Receiver<()>> {
        self.0
            .notices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    pub fn holds_application(&self) -> bool {
        self.0
            .activity
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .active
            > 0
    }

    /// Close admission only after actual decoder teardown, not a stop request.
    pub fn shutdown(&self) -> io::Result<()> {
        let mut activity = self.0.activity.lock().unwrap_or_else(|e| e.into_inner());
        if activity.active != 0 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Media playback is still closing.",
            ));
        }
        activity.closed = true;
        Ok(())
    }
}

struct WorkerActivity(Arc<Runtime>);
impl Drop for WorkerActivity {
    fn drop(&mut self) {
        self.0
            .activity
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .active -= 1;
        self.0.notify();
    }
}

struct Shared {
    snapshot: Mutex<Snapshot>,
    cancellation: Cancellation,
    runtime: Arc<Runtime>,
}
impl Shared {
    fn publish(&self, mut snapshot: Snapshot) {
        let mut current = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        snapshot.revision = current.revision + 1;
        *current = snapshot;
        drop(current);
        self.runtime.notify();
    }
}

/// A session is deliberately not cloneable: its owner controls its lifetime.
/// Observing a snapshot retains at most its one frame, never the decoder.
pub struct Session {
    shared: Arc<Shared>,
    commands: mpsc::SyncSender<Command>,
}
impl Session {
    pub fn snapshot(&self) -> Snapshot {
        self.shared
            .snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// A bounded command mailbox preserves order and reports saturation.
    pub fn request(&self, command: Command) -> io::Result<()> {
        if matches!(command, Command::Volume(v) if !v.is_finite() || !(0.0..=1.0).contains(&v))
            || matches!(command, Command::VolumeBy(v) if !v.is_finite() || !(-1.0..=1.0).contains(&v))
            || matches!(command, Command::Seek(position) if position.as_nanos() >= u64::MAX as u128)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid media control value.",
            ));
        }
        if self.shared.cancellation.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Media playback has stopped.",
            ));
        }
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => {
                    io::Error::new(io::ErrorKind::WouldBlock, "Media controls are busy.")
                }
                mpsc::TrySendError::Disconnected(_) => {
                    io::Error::new(io::ErrorKind::BrokenPipe, "Media playback has stopped.")
                }
            })
    }

    /// Cancellation is immediate; the runtime holds the worker through teardown.
    pub fn stop(&self) {
        self.shared.cancellation.cancel();
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}
