//! Audio observation and control shared by the bar and audio panel provider.
use tokio::sync::mpsc;

mod devices;
mod volume;
pub use devices::{
    AudioControl, AudioDevice, AudioDeviceId, AudioSnapshot, DeviceKind, DeviceOperation, monitor,
};

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
    fn connect(server: Option<&str>, cancelled: impl Fn() -> bool) -> Result<Self, String> {
        let mut mainloop =
            pulse::mainloop::threaded::Mainloop::new().ok_or("cannot create audio mainloop")?;
        let context = pulse::context::Context::new(&mainloop, "canopa-shell")
            .ok_or("cannot create audio context")?;
        mainloop
            .start()
            .map_err(|error| format!("audio mainloop: {error:?}"))?;
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
        result.map_err(|error| format!("audio connection: {error:?}"))?;
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            connection.mainloop.lock();
            let state = connection.context.borrow().get_state();
            connection.mainloop.unlock();
            match state {
                pulse::context::State::Ready => return Ok(connection),
                pulse::context::State::Failed | pulse::context::State::Terminated => {
                    return Err("audio server unavailable".into());
                }
                _ if cancelled() || Instant::now() >= deadline => {
                    return Err("audio connection cancelled or timed out".into());
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

pub async fn adjust_default_sink_volume(step: u8, up: bool) -> Result<(), String> {
    // Serialize read/modify/write so rapid scroll events do not lose increments.
    static ADJUSTMENT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = ADJUSTMENT.lock().await;
    tokio::task::spawn_blocking(move || adjust_volume_on(None, step, up))
        .await
        .map_err(|error| error.to_string())?
}

fn adjust_volume_on(server: Option<&str>, step: u8, up: bool) -> Result<(), String> {
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
                            Err("audio server rejected volume adjustment".into())
                        });
                    })),
                );
            }
            ListResult::Error => {
                let _ = tx.send(Err("default audio sink is unavailable".into()));
            }
            ListResult::End if !found => {
                let _ = tx.send(Err("default audio sink is unavailable".into()));
            }
            ListResult::End => {}
        });
    connection.mainloop.unlock();
    rx.recv_timeout(Duration::from_secs(2))
        .map_err(|_| "audio adjustment timed out".to_owned())?
}
