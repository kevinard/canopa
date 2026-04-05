use crate::modules::volume::VolumeEvent;
use iced::futures::stream;
use tokio::sync::mpsc;

struct VolumeConnData;

impl std::hash::Hash for VolumeConnData {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        "volume-subscription".hash(h);
    }
}

enum SubState {
    Init,
    Running(mpsc::UnboundedReceiver<Result<(f64, bool), ()>>),
    Done,
}

fn build_volume_stream(
    _data: &VolumeConnData,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = crate::app::Message> + Send + 'static>> {
    Box::pin(stream::unfold(SubState::Init, |state| async move {
        match state {
            SubState::Init => {
                let (tx, rx) = mpsc::unbounded_channel();
                std::thread::spawn(move || run_pa_thread(tx));
                let mut rx = rx;
                match rx.recv().await {
                    Some(Ok((v, m))) => Some((
                        crate::app::Message::Volume(VolumeEvent::StateChanged {
                            volume: v,
                            muted: m,
                        }),
                        SubState::Running(rx),
                    )),
                    _ => Some((
                        crate::app::Message::Volume(VolumeEvent::Unavailable),
                        SubState::Done,
                    )),
                }
            }
            SubState::Running(mut rx) => match rx.recv().await {
                Some(Ok((v, m))) => Some((
                    crate::app::Message::Volume(VolumeEvent::StateChanged {
                        volume: v,
                        muted: m,
                    }),
                    SubState::Running(rx),
                )),
                _ => Some((
                    crate::app::Message::Volume(VolumeEvent::Unavailable),
                    SubState::Done,
                )),
            },
            SubState::Done => None,
        }
    }))
}

/// Returns an Iced subscription for volume state via PulseAudio compat layer.
/// Emits Unavailable if PA socket is not reachable. No zbus connection needed.
pub fn volume_subscription() -> iced::Subscription<crate::app::Message> {
    iced::Subscription::run_with(VolumeConnData, build_volume_stream)
}

/// Runs in a dedicated std::thread. Sets up the PA threaded mainloop,
/// sends initial state, subscribes to sink changes, and blocks until PA terminates.
fn run_pa_thread(tx: mpsc::UnboundedSender<Result<(f64, bool), ()>>) {
    use libpulse_binding as pulse;
    use pulse::context::subscribe::{Facility, InterestMaskSet, Operation};
    use pulse::mainloop::threaded::Mainloop;
    use std::cell::RefCell;
    use std::rc::Rc;

    let mut mainloop = match Mainloop::new() {
        Some(m) => m,
        None => {
            let _ = tx.send(Err(()));
            return;
        }
    };

    let mut context = match pulse::context::Context::new(&mainloop, "niribar") {
        Some(c) => c,
        None => {
            let _ = tx.send(Err(()));
            return;
        }
    };

    // PA threaded mainloop pattern:
    // 1. Install a state callback that calls mainloop.signal() when context state changes.
    //    Without this, mainloop.wait() blocks forever — the PA background thread never wakes
    //    up the waiting main thread.
    // 2. Start the mainloop background thread.
    // 3. Lock, connect, wait for Ready using the lock/wait/signal protocol.
    //
    // mainloop stays on the stack for the entire function lifetime; the state callback uses
    // a raw pointer to avoid RefCell borrow conflicts (wait() holds exclusive access while
    // sleeping, which would prevent signal() from acquiring a borrow_mut()).
    let ml_ptr = &mut mainloop as *mut Mainloop as usize;
    context.set_state_callback(Some(Box::new(move || {
        // SAFETY: mainloop lives on this thread's stack for the entire run_pa_thread call.
        // PA guarantees that signal() is only called from within the PA thread while we hold
        // the mainloop lock, and wait() atomically releases the lock before sleeping.
        let ml = unsafe { &mut *(ml_ptr as *mut Mainloop) };
        ml.signal(false);
    })));

    if mainloop.start().is_err() {
        let _ = tx.send(Err(()));
        return;
    }

    mainloop.lock();
    if context
        .connect(None, pulse::context::FlagSet::NOAUTOSPAWN, None)
        .is_err()
    {
        mainloop.unlock();
        let _ = tx.send(Err(()));
        return;
    }

    // Wait for context Ready. mainloop.wait() atomically releases the lock, sleeps until
    // signal() is called from within the PA background thread, then re-acquires the lock.
    loop {
        match context.get_state() {
            pulse::context::State::Ready => break,
            pulse::context::State::Failed | pulse::context::State::Terminated => {
                mainloop.unlock();
                let _ = tx.send(Err(()));
                return;
            }
            _ => mainloop.wait(),
        }
    }

    // Clear state callback — no longer needed after Ready.
    context.set_state_callback(None);

    // Send initial volume state.
    fetch_volume(&context, &tx);

    // Wrap context in Rc<RefCell> now that we need to share it with the subscribe callback.
    let context = Rc::new(RefCell::new(context));

    // Subscribe to sink + server changes so we get notified when volume changes.
    context
        .borrow_mut()
        .subscribe(InterestMaskSet::SINK | InterestMaskSet::SERVER, |_| {});
    let tx_cb = tx.clone();
    let ctx_cb = Rc::clone(&context);
    context
        .borrow_mut()
        .set_subscribe_callback(Some(Box::new(move |facility, op, _idx| {
            if let (
                Some(Facility::Sink) | Some(Facility::Server),
                Some(Operation::Changed) | Some(Operation::New),
            ) = (facility, op)
            {
                fetch_volume(&ctx_cb.borrow(), &tx_cb);
            }
        })));

    mainloop.unlock();

    // Block this thread until PA terminates (PA mainloop runs in its own thread).
    loop {
        std::thread::sleep(std::time::Duration::from_secs(5));
        mainloop.lock();
        let state = context.borrow().get_state();
        mainloop.unlock();
        if matches!(
            state,
            pulse::context::State::Failed | pulse::context::State::Terminated
        ) {
            break;
        }
    }
    let _ = tx.send(Err(())); // Signal stream end
}

/// Fetch the default sink volume and mute state via PA introspect API.
/// Called both for initial state and in subscribe callback.
fn fetch_volume(
    ctx: &libpulse_binding::context::Context,
    tx: &mpsc::UnboundedSender<Result<(f64, bool), ()>>,
) {
    let tx2 = tx.clone();
    ctx.introspect()
        .get_sink_info_by_name("@DEFAULT_SINK@", move |res| {
            use libpulse_binding::callbacks::ListResult;
            if let ListResult::Item(sink) = res {
                let raw = sink.volume.avg().0 as f64;
                let norm = libpulse_binding::volume::Volume::NORMAL.0 as f64;
                let pct = (raw / norm * 100.0).clamp(0.0, 150.0);
                let _ = tx2.send(Ok((pct, sink.mute)));
            }
        });
}

#[cfg(test)]
mod tests {
    // pipewire-pulse maps PipeWire linear amplitude directly to PA volume:
    // pa_volume = linear * PA_VOLUME_NORM, so pct = pa_volume / NORM * 100 = linear * 100.

    #[test]
    fn pa_volume_zero_maps_to_0_pct() {
        let (raw, norm) = (0.0_f64, 65536.0_f64);
        let pct = (raw / norm * 100.0).clamp(0.0, 150.0);
        assert_eq!(pct, 0.0);
    }

    #[test]
    fn pa_volume_norm_maps_to_100_pct() {
        let (raw, norm) = (65536.0_f64, 65536.0_f64);
        let pct = (raw / norm * 100.0).clamp(0.0, 150.0);
        assert_eq!(pct, 100.0);
    }

    #[test]
    fn pa_volume_over_norm_clamped_to_150() {
        let (raw, norm) = (65536.0_f64 * 2.0, 65536.0_f64);
        let pct = (raw / norm * 100.0).clamp(0.0, 150.0);
        assert_eq!(pct, 150.0);
    }
}
