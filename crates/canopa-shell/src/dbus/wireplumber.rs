use crate::modules::volume::VolumeEvent;
use tokio::sync::mpsc;

#[cfg(test)]
#[path = "wireplumber_tests.rs"]
mod transport_tests;

struct VolumeConnData;

impl std::hash::Hash for VolumeConnData {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        "volume-subscription".hash(h);
    }
}

fn build_volume_stream(
    _data: &VolumeConnData,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = crate::app::Message> + Send + 'static>> {
    crate::dbus::subscription::build::<Option<(f64, bool)>, crate::app::Message, _, _, _>(
        |tx| {
            std::thread::spawn(move || run_pa_thread(tx));
        },
        |reading| {
            let Some((volume, muted)) = reading else {
                return crate::app::Message::Modules(
                    crate::app::messages::ModuleRuntimeEvent::Volume(VolumeEvent::Unavailable),
                );
            };
            crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Volume(
                VolumeEvent::StateChanged { volume, muted },
            ))
        },
        || {
            crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Volume(
                VolumeEvent::Unavailable,
            ))
        },
    )
}

/// Returns an Iced subscription for volume state via PulseAudio compat layer.
/// Emits Unavailable if PA socket is not reachable. No zbus connection needed.
pub(crate) fn volume_subscription() -> iced::Subscription<crate::app::Message> {
    iced::Subscription::run_with(VolumeConnData, build_volume_stream)
}

// Both observation and control use the same PulseAudio protocol/server selection.
// PipeWire's PulseAudio compatibility server implements that same contract.
use libpulse_binding as pulse;
use std::{
    cell::RefCell,
    rc::Rc,
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
        self.context.borrow_mut().disconnect();
        self.mainloop.unlock();
        self.mainloop.stop();
    }
}

fn run_pa_thread(tx: VolumeSender) {
    run_pa_thread_on(None, tx);
}

fn run_pa_thread_on(server: Option<&str>, tx: VolumeSender) {
    use pulse::context::subscribe::{Facility, InterestMaskSet};
    let Ok(mut connection) = PulseConnection::connect(server, || tx.is_closed()) else {
        let _ = tx.send(Err(()));
        return;
    };
    connection.mainloop.lock();
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
    loop {
        std::thread::sleep(Duration::from_millis(50));
        if tx.is_closed() {
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

pub(crate) async fn adjust_default_sink_volume(step: u8, up: bool) -> Result<(), String> {
    // Serialize read/modify/write so rapid scroll events do not lose increments.
    static ADJUSTMENT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = ADJUSTMENT.lock().await;
    tokio::task::spawn_blocking(move || adjust_volume_on(None, step, up))
        .await
        .map_err(|error| error.to_string())?
}

fn adjust_volume_on(server: Option<&str>, step: u8, up: bool) -> Result<(), String> {
    use pulse::callbacks::ListResult;
    use pulse::volume::Volume;
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
                let mut volume = sink.volume;
                let delta = Volume((u64::from(Volume::NORMAL.0) * u64::from(step) / 100) as u32);
                let adjusted = if up {
                    volume.inc_clamp(delta, Volume::NORMAL)
                } else {
                    volume.decrease(delta)
                };
                if adjusted.is_none() {
                    let _ = tx.send(Err("invalid sink volume".into()));
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
