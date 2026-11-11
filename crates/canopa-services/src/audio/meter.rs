//! A bounded input level meter: the server's peak detection on a recording
//! stream of 25 single-channel samples per second. No audio is kept; each
//! peak replaces the previous one. Dropping the receiver ends the stream.
use super::{AudioError, PulseConnection, devices::METER_APPLICATION, pulse};
use std::{cell::RefCell, rc::Rc, time::Duration};
use tokio::sync::watch;

/// Peaks per second the server sends.
const RATE: u32 = 25;

/// Follow the peak level, 0.0 to 1.0, of the input device `source` (a source
/// name, or `None` for the default input) on `server` (`None`: the default
/// server). The receiver reads `None` until the first peak and after a
/// failure; the meter does not reconnect, so open a new one to retry.
pub fn level_meter(server: Option<String>, source: Option<String>) -> watch::Receiver<Option<f32>> {
    let (tx, rx) = watch::channel(None);
    std::thread::spawn(move || {
        let _ = run(server.as_deref(), source.as_deref(), &tx);
        tx.send_replace(None);
    });
    rx
}

fn run(
    server: Option<&str>,
    source: Option<&str>,
    tx: &watch::Sender<Option<f32>>,
) -> Result<(), AudioError> {
    use pulse::{
        def::BufferAttr,
        sample::{Format, Spec},
        stream::{FlagSet, PeekResult, State, Stream},
    };
    let mut connection = PulseConnection::connect(server, || tx.is_closed())?;
    let spec = Spec {
        format: Format::FLOAT32NE,
        channels: 1,
        rate: RATE,
    };
    let mut properties =
        pulse::proplist::Proplist::new().ok_or(AudioError::Connection("proplist".into()))?;
    let _ = properties.set_str("application.id", METER_APPLICATION);
    connection.mainloop.lock();
    let stream = Stream::new_with_proplist(
        &mut connection.context.borrow_mut(),
        "Input level",
        &spec,
        None,
        &mut properties,
    );
    let Some(stream) = stream else {
        connection.mainloop.unlock();
        return Err(AudioError::Connection(
            "cannot create the level stream".into(),
        ));
    };
    let stream = Rc::new(RefCell::new(stream));
    let reader = Rc::clone(&stream);
    let peaks = tx.clone();
    stream
        .borrow_mut()
        .set_read_callback(Some(Box::new(move |_| {
            let mut stream = reader.borrow_mut();
            let mut peak = None;
            while let Ok(result) = stream.peek() {
                match result {
                    PeekResult::Empty => break,
                    PeekResult::Hole(_) => {}
                    PeekResult::Data(bytes) => {
                        if let Some(sample) = bytes.rchunks_exact(4).next() {
                            let value = f32::from_ne_bytes(sample.try_into().unwrap());
                            peak = Some(value.abs().clamp(0.0, 1.0));
                        }
                    }
                }
                if stream.discard().is_err() {
                    break;
                }
            }
            if let Some(peak) = peak {
                peaks.send_replace(Some(peak));
            }
        })));
    let attributes = BufferAttr {
        maxlength: u32::MAX,
        tlength: u32::MAX,
        prebuf: u32::MAX,
        minreq: u32::MAX,
        fragsize: 4,
    };
    let connected = stream.borrow_mut().connect_record(
        source,
        Some(&attributes),
        FlagSet::PEAK_DETECT | FlagSet::ADJUST_LATENCY | FlagSet::DONT_MOVE,
    );
    connection.mainloop.unlock();
    connected.map_err(|error| AudioError::Connection(format!("level stream: {error:?}")))?;
    let result = loop {
        if tx.is_closed() {
            break Ok(());
        }
        connection.mainloop.lock();
        let state = stream.borrow().get_state();
        let context = connection.context.borrow().get_state();
        connection.mainloop.unlock();
        if matches!(state, State::Failed | State::Terminated)
            || context != pulse::context::State::Ready
        {
            break Err(AudioError::Unavailable);
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    connection.mainloop.lock();
    stream.borrow_mut().set_read_callback(None);
    let _ = stream.borrow_mut().disconnect();
    connection.mainloop.unlock();
    result
}
