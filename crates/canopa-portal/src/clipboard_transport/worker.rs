use super::{pipes::Pump, *};
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    net,
};
use std::{collections::BTreeMap, io::Read, path::PathBuf, sync::Mutex, time::Instant};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    protocol::{wl_callback, wl_registry, wl_seat},
};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1 as device, ext_data_control_manager_v1 as manager,
    ext_data_control_offer_v1 as offer, ext_data_control_source_v1 as source,
};

fn error(value: impl std::fmt::Display) -> io::Error {
    io::Error::other(value.to_string())
}
fn connection(pid: u32) -> io::Result<Connection> {
    let display = std::env::var_os("WAYLAND_DISPLAY")
        .map(PathBuf::from)
        .ok_or_else(|| error("Clipboard needs a named Wayland display"))?;
    let path = if display.is_absolute() {
        display
    } else {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(|| error("Invalid Wayland runtime directory"))?;
        runtime.join(display)
    };
    let socket = net::socket_with(
        net::AddressFamily::UNIX,
        net::SocketType::STREAM,
        net::SocketFlags::NONBLOCK | net::SocketFlags::CLOEXEC,
        None,
    )?;
    net::connect(&socket, &net::SocketAddrUnix::new(path)?)?;
    if u32::try_from(
        net::sockopt::socket_peercred(&socket)?
            .pid
            .as_raw_nonzero()
            .get(),
    )
    .ok()
        != Some(pid)
    {
        return Err(error("Clipboard display belongs to another compositor"));
    }
    Connection::from_socket(UnixStream::from(socket)).map_err(error)
}

#[derive(Default)]
struct Types {
    values: Vec<String>,
    invalid: bool,
}
enum Callback {
    Registry,
    Device,
    Published(u64),
}
struct Source {
    proxy: source::ExtDataControlSourceV1,
    identity: u64,
    marker: String,
    types: Vec<String>,
}
struct Publication {
    identity: u64,
    reply: Reply<()>,
    expires: Instant,
}
enum Phase {
    Pending(OwnedFd),
    Writing { pump: Box<Pump>, done: bool },
}
struct Write {
    source: u64,
    phase: Phase,
    expires: Instant,
}
struct Worker {
    snapshots: watch::Sender<Snapshot>,
    events: asynchronous::Sender<Transfer>,
    manager: Option<manager::ExtDataControlManagerV1>,
    manager_name: Option<u32>,
    seat: Option<wl_seat::WlSeat>,
    seat_name: Option<u32>,
    device: Option<device::ExtDataControlDeviceV1>,
    offer: Option<offer::ExtDataControlOfferV1>,
    source: Option<Source>,
    publication: Option<Publication>,
    writes: BTreeMap<u32, Write>,
    reads: BTreeMap<u64, Pump>,
    nonce: String,
    next: u64,
    serials: Serials,
    generation: u64,
    ready: bool,
    started: Instant,
    failure: Option<String>,
}
impl Worker {
    fn next(&mut self) -> io::Result<u64> {
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| error("Clipboard identities exhausted"))?;
        Ok(self.next)
    }
    fn types(&self) -> io::Result<Vec<String>> {
        let Some(offer) = &self.offer else {
            return Ok(Vec::new());
        };
        let types = offer
            .data::<Mutex<Types>>()
            .expect("clipboard offer data")
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if types.invalid {
            return Err(error("Clipboard offer exceeds metadata limits"));
        }
        let values: Vec<_> = types
            .values
            .iter()
            .filter(|mime| !mime.starts_with(OWNER_PREFIX))
            .cloned()
            .collect();
        validate_types(&values).map_err(error)?;
        Ok(values)
    }
    fn owned(&self) -> bool {
        self.source.as_ref().is_some_and(|source| {
            source.proxy.is_alive()
                && self.offer.as_ref().is_some_and(|offer| {
                    offer
                        .data::<Mutex<Types>>()
                        .expect("clipboard offer data")
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .values
                        .contains(&source.marker)
                })
        })
    }
    fn snapshot(&mut self) {
        match self.types() {
            Ok(types) => {
                self.snapshots.send_replace(Snapshot {
                    ready: self.ready,
                    generation: self.generation,
                    mime_types: types,
                    session_is_owner: self.owned(),
                    error: None,
                });
            }
            Err(error) => self.failure = Some(error.to_string()),
        }
    }
    fn publish(
        &mut self,
        types: Vec<String>,
        reply: Reply<()>,
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if self.publication.is_some() {
            let _ = reply.send(Err("Clipboard publication pending".into()));
            return;
        }
        let prepared: io::Result<u64> = (|| {
            validate_types(&types).map_err(error)?;
            let identity = self.next()?;
            if let Some(source) = self.source.take() {
                source.proxy.destroy();
            }
            self.writes.clear();
            if types.is_empty() {
                self.device
                    .as_ref()
                    .expect("clipboard device")
                    .set_selection(None);
            } else {
                let marker = format!("{OWNER_PREFIX}{}-{identity}", self.nonce);
                let proxy = self
                    .manager
                    .as_ref()
                    .expect("clipboard manager")
                    .create_data_source(qh, identity);
                for mime in &types {
                    proxy.offer(mime.clone());
                }
                proxy.offer(marker.clone());
                self.device
                    .as_ref()
                    .expect("clipboard device")
                    .set_selection(Some(&proxy));
                self.source = Some(Source {
                    proxy,
                    identity,
                    marker,
                    types,
                });
            }
            conn.display().sync(qh, Callback::Published(identity));
            Ok(identity)
        })();
        match prepared {
            Ok(identity) => {
                self.publication = Some(Publication {
                    identity,
                    reply,
                    expires: Instant::now() + STARTUP,
                })
            }
            Err(error) => {
                let _ = reply.send(Err(error.to_string()));
            }
        }
    }
    fn read(&mut self, generation: u64, mime: String) -> io::Result<OwnedFd> {
        validate_mime(&mime).map_err(error)?;
        if generation != self.generation || !self.types()?.contains(&mime) {
            return Err(error("Clipboard selection changed or format unavailable"));
        }
        if self.reads.len() + self.writes.len() >= MAX_TRANSFERS {
            return Err(error("Too many clipboard transfers"));
        }
        let id = self.next()?;
        let (source_reader, source_writer) = pipes::pipe()?;
        let (app_reader, app_writer) = pipes::pipe()?;
        let pump = Pump::new(source_reader, app_writer)?;
        self.offer
            .as_ref()
            .ok_or_else(|| error("Clipboard is empty"))?
            .receive(mime, std::os::fd::AsFd::as_fd(&source_writer));
        self.reads.insert(id, pump);
        Ok(app_reader)
    }
    fn write(&mut self, serial: u32) -> io::Result<OwnedFd> {
        let transfer = self
            .writes
            .remove(&serial)
            .ok_or_else(|| error("Unknown or expired clipboard serial"))?;
        let Phase::Pending(target) = transfer.phase else {
            self.writes.insert(serial, transfer);
            return Err(error("Clipboard serial already claimed"));
        };
        let (reader, writer) = pipes::pipe()?;
        let pump = Pump::new(reader, target)?;
        self.writes.insert(
            serial,
            Write {
                source: transfer.source,
                phase: Phase::Writing {
                    pump: Box::new(pump),
                    done: false,
                },
                expires: Instant::now() + LIFETIME,
            },
        );
        Ok(writer)
    }
    fn done(&mut self, serial: u32, success: bool) -> io::Result<()> {
        let transfer = self
            .writes
            .get_mut(&serial)
            .ok_or_else(|| error("Unknown or expired clipboard serial"))?;
        if let Phase::Writing { done: true, .. } = transfer.phase {
            return Err(error("Clipboard serial already completed"));
        }
        if !success {
            self.writes.remove(&serial);
            return Ok(());
        }
        match &mut transfer.phase {
            Phase::Writing { done, .. } => {
                *done = true;
                Ok(())
            }
            Phase::Pending(_) => Err(error("Clipboard serial has not been written")),
        }
    }
    fn pump(&mut self) {
        let now = Instant::now();
        self.reads
            .retain(|_, pump| now < pump.deadline() && matches!(pump.step(), Ok(false)));
        self.writes.retain(|_, write| {
            if now >= write.expires {
                return false;
            }
            match &mut write.phase {
                Phase::Pending(_) => true,
                Phase::Writing { pump, done } => {
                    now < pump.deadline()
                        && match pump.step() {
                            Ok(eof) => !eof || !*done,
                            Err(_) => false,
                        }
                }
            }
        });
    }
    fn command(&mut self, command: Command, conn: &Connection, qh: &QueueHandle<Self>) {
        match command {
            Command::Publish(types, reply) => self.publish(types, reply, conn, qh),
            Command::Read(generation, mime, reply) => {
                let result = self.read(generation, mime).map_err(|e| e.to_string());
                let _ = reply.send(result);
            }
            Command::Write(serial, reply) => {
                let result = self.write(serial).map_err(|e| e.to_string());
                let _ = reply.send(result);
            }
            Command::Done(serial, success, reply) => {
                let result = self.done(serial, success).map_err(|e| e.to_string());
                let _ = reply.send(result);
            }
        }
    }
    fn deadline(&self) -> Instant {
        let mut deadline = Instant::now() + IDLE;
        if !self.ready {
            deadline = deadline.min(self.started + STARTUP);
        }
        if let Some(publication) = &self.publication {
            deadline = deadline.min(publication.expires);
        }
        for pump in self.reads.values() {
            deadline = deadline.min(pump.deadline());
        }
        for write in self.writes.values() {
            deadline = deadline.min(write.expires);
            if let Phase::Writing { pump, .. } = &write.phase {
                deadline = deadline.min(pump.deadline());
            }
        }
        deadline
    }
}

pub(super) fn run(
    pid: u32,
    receiver: mpsc::Receiver<Command>,
    mut wake: UnixStream,
    snapshots: watch::Sender<Snapshot>,
    events: asynchronous::Sender<Transfer>,
    closing: Arc<AtomicBool>,
    serials: Serials,
) -> io::Result<()> {
    let conn = connection(pid)?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let mut nonce = [0u8; 16];
    if rustix::rand::getrandom(&mut nonce, rustix::rand::GetRandomFlags::empty())? != nonce.len() {
        return Err(error("Could not create clipboard identity"));
    }
    let mut state = Worker {
        snapshots,
        events,
        manager: None,
        manager_name: None,
        seat: None,
        seat_name: None,
        device: None,
        offer: None,
        source: None,
        publication: None,
        writes: BTreeMap::new(),
        reads: BTreeMap::new(),
        nonce: nonce.iter().map(|byte| format!("{byte:02x}")).collect(),
        next: 0,
        serials,
        generation: 0,
        ready: false,
        started: Instant::now(),
        failure: None,
    };
    let _registry = conn.display().get_registry(&qh, ());
    conn.display().sync(&qh, Callback::Registry);
    dispatch(
        &receiver, &conn, &mut queue, &qh, &mut state, &mut wake, &closing,
    )
}
fn dispatch(
    receiver: &mpsc::Receiver<Command>,
    conn: &Connection,
    queue: &mut EventQueue<Worker>,
    qh: &QueueHandle<Worker>,
    state: &mut Worker,
    wake: &mut UnixStream,
    closing: &AtomicBool,
) -> io::Result<()> {
    loop {
        if closing.load(Ordering::Acquire) {
            return Ok(());
        }
        queue.dispatch_pending(state).map_err(error)?;
        if let Some(failure) = state.failure.take() {
            return Err(error(failure));
        }
        if !state.ready && state.started.elapsed() >= STARTUP {
            return Err(error("Clipboard compositor did not initialize"));
        }
        if state
            .publication
            .as_ref()
            .is_some_and(|value| Instant::now() >= value.expires)
        {
            return Err(error("Clipboard publication timed out"));
        }
        for _ in 0..8 {
            if closing.load(Ordering::Acquire) {
                return Ok(());
            }
            match receiver.try_recv() {
                Ok(command) => state.command(command, conn, qh),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }
        state.pump();
        let write_pending = match queue.flush() {
            Ok(()) => false,
            Err(wayland_client::backend::WaylandError::Io(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                true
            }
            Err(failure) => return Err(error(failure)),
        };
        let Some(guard) = queue.prepare_read() else {
            continue;
        };
        let mut fds = vec![
            PollFd::new(
                &*queue,
                PollFlags::IN
                    | if write_pending {
                        PollFlags::OUT
                    } else {
                        PollFlags::empty()
                    },
            ),
            PollFd::new(&*wake, PollFlags::IN),
        ];
        let pumps =
            state.reads.values().chain(state.writes.values().filter_map(
                |write| match &write.phase {
                    Phase::Writing { pump, .. } => Some(pump.as_ref()),
                    _ => None,
                },
            ));
        for pump in pumps {
            if pump.wants_read() {
                fds.push(PollFd::new(&pump.reader, PollFlags::IN));
            }
            if pump.wants_write() {
                fds.push(PollFd::new(&pump.writer, PollFlags::OUT));
            }
        }
        let timeout =
            Timespec::try_from(state.deadline().saturating_duration_since(Instant::now()))
                .map_err(error)?;
        match poll(&mut fds, Some(&timeout)) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => {
                drop(guard);
                continue;
            }
            Err(failure) => return Err(failure.into()),
        }
        let readable = fds[0]
            .revents()
            .intersects(PollFlags::IN | PollFlags::HUP | PollFlags::ERR);
        drop(fds);
        if readable {
            match guard.read() {
                Ok(_) => {}
                Err(wayland_client::backend::WaylandError::Io(error))
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(failure) => return Err(error(failure)),
            }
        } else {
            drop(guard);
        }
        let mut bytes = [0; 64];
        while matches!(wake.read(&mut bytes), Ok(count) if count > 0) {}
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Worker {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name, interface, ..
            } if interface == "ext_data_control_manager_v1" && state.manager.is_none() => {
                state.manager = Some(registry.bind(name, 1, qh, ()));
                state.manager_name = Some(name);
            }
            wl_registry::Event::Global {
                name, interface, ..
            } if interface == "wl_seat" => {
                if state.seat.is_some() {
                    state.failure = Some("Clipboard requires an unambiguous seat".into());
                } else {
                    state.seat = Some(registry.bind(name, 1, qh, ()));
                    state.seat_name = Some(name);
                }
            }
            wl_registry::Event::GlobalRemove { name }
                if state.seat_name == Some(name) || state.manager_name == Some(name) =>
            {
                state.failure = Some("Clipboard seat or manager disappeared".into());
            }
            _ => {}
        }
    }
}
wayland_client::delegate_noop!(Worker: ignore manager::ExtDataControlManagerV1);
wayland_client::delegate_noop!(Worker: ignore wl_seat::WlSeat);
impl Dispatch<wl_callback::WlCallback, Callback> for Worker {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        _: wl_callback::Event,
        data: &Callback,
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match data {
            Callback::Registry => match (&state.manager, &state.seat) {
                (Some(manager), Some(seat)) => {
                    state.device = Some(manager.get_data_device(seat, qh, ()));
                    conn.display().sync(qh, Callback::Device);
                }
                _ => state.failure = Some("Compositor lacks clipboard data-control".into()),
            },
            Callback::Device => {
                state.ready = true;
                state.snapshot();
            }
            Callback::Published(id) => {
                if let Some(publication) = state.publication.take() {
                    let accepted = publication.identity == *id
                        && match &state.source {
                            Some(source) => source.identity == *id && state.owned(),
                            None => state.offer.is_none(),
                        };
                    let _ = publication.reply.send(if accepted {
                        Ok(())
                    } else {
                        Err("Clipboard publication was superseded".into())
                    });
                }
            }
        }
    }
}
impl Dispatch<device::ExtDataControlDeviceV1, ()> for Worker {
    fn event(
        state: &mut Self,
        _: &device::ExtDataControlDeviceV1,
        event: device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            device::Event::Selection { id } => {
                if let Some(old) = state.offer.take() {
                    old.destroy();
                }
                state.offer = id;
                state.reads.clear();
                state.writes.clear();
                if let Some(generation) = state.generation.checked_add(1) {
                    state.generation = generation;
                    state.snapshot();
                } else {
                    state.failure = Some("Clipboard generations exhausted".into());
                }
            }
            device::Event::PrimarySelection { id: Some(offer) } => offer.destroy(),
            device::Event::Finished => {
                state.failure = Some("Clipboard data-control disconnected".into())
            }
            _ => {}
        }
    }
    wayland_client::event_created_child!(Worker, device::ExtDataControlDeviceV1, [0 => (offer::ExtDataControlOfferV1, Mutex::new(Types::default()))]);
}
impl Dispatch<offer::ExtDataControlOfferV1, Mutex<Types>> for Worker {
    fn event(
        _: &mut Self,
        _: &offer::ExtDataControlOfferV1,
        event: offer::Event,
        data: &Mutex<Types>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let offer::Event::Offer { mime_type } = event {
            let mut types = data.lock().unwrap_or_else(|e| e.into_inner());
            if types.values.len() > MAX_TYPES
                || mime_type.len() > MAX_MIME_BYTES
                || mime_type.chars().any(char::is_control)
                || mime_type.is_empty()
            {
                types.invalid = true;
            } else if !types.values.contains(&mime_type) {
                types.values.push(mime_type);
            }
        }
    }
}
impl Dispatch<source::ExtDataControlSourceV1, u64> for Worker {
    fn event(
        state: &mut Self,
        proxy: &source::ExtDataControlSourceV1,
        event: source::Event,
        identity: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            source::Event::Cancelled => {
                if state
                    .source
                    .as_ref()
                    .is_some_and(|source| source.identity == *identity)
                {
                    state.source = None;
                    state.snapshot();
                }
                state.writes.retain(|_, write| write.source != *identity);
                if proxy.is_alive() {
                    proxy.destroy();
                }
            }
            source::Event::Send { mime_type, fd } => {
                let Some(source) = &state.source else {
                    return;
                };
                if source.identity != *identity
                    || !source.types.contains(&mime_type)
                    || state.writes.len() + state.reads.len() >= MAX_TRANSFERS
                {
                    return;
                }
                let Some(serial) = state.serials.next() else {
                    state.failure = Some("Clipboard serials exhausted".into());
                    return;
                };
                if state
                    .events
                    .try_send(Transfer {
                        generation: state.generation,
                        serial,
                        mime_type,
                    })
                    .is_ok()
                {
                    state.writes.insert(
                        serial,
                        Write {
                            source: *identity,
                            phase: Phase::Pending(fd),
                            expires: Instant::now() + IDLE,
                        },
                    );
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::File, io::Write as _};
    fn state() -> Worker {
        let (snapshots, _) = watch::channel(Snapshot::default());
        let (events, _) = asynchronous::channel(MAX_TRANSFERS);
        Worker {
            snapshots,
            events,
            manager: None,
            manager_name: None,
            seat: None,
            seat_name: None,
            device: None,
            offer: None,
            source: None,
            publication: None,
            writes: BTreeMap::new(),
            reads: BTreeMap::new(),
            nonce: "test".into(),
            next: 0,
            serials: Serials::default(),
            generation: 0,
            ready: true,
            started: Instant::now(),
            failure: None,
        }
    }
    fn pending(worker: &mut Worker, serial: u32) -> File {
        let (reader, target) = pipes::pipe().unwrap();
        worker.writes.insert(
            serial,
            Write {
                source: 1,
                phase: Phase::Pending(target),
                expires: Instant::now() + IDLE,
            },
        );
        File::from(reader)
    }
    #[test]
    fn transfer_serials_are_consumed_once_and_completion_waits_for_eof() {
        let mut worker = state();
        let mut target = pending(&mut worker, 7);
        rustix::fs::fcntl_setfl(&target, rustix::fs::OFlags::NONBLOCK).unwrap();
        assert!(worker.write(8).is_err());
        assert!(worker.done(7, true).is_err());
        let mut writer = File::from(worker.write(7).unwrap());
        assert!(worker.write(7).is_err());
        writer.write_all(b"exact bytes").unwrap();
        drop(writer);
        for _ in 0..4 {
            worker.pump();
        }
        let mut bytes = [0; 64];
        let count = target.read(&mut bytes).unwrap();
        assert_eq!(&bytes[..count], b"exact bytes");
        // EOF is withheld until the application reports successful completion.
        assert_eq!(
            target.read(&mut bytes).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(worker.writes.contains_key(&7));
        worker.done(7, true).unwrap();
        assert!(worker.done(7, true).is_err());
        worker.pump();
        assert_eq!(target.read(&mut bytes).unwrap(), 0);
        assert!(worker.write(7).is_err());
        assert!(worker.done(7, false).is_err());
    }
    #[test]
    fn declined_unanswered_and_expired_transfers_close_the_peer() {
        let mut worker = state();
        let mut declined = pending(&mut worker, 1);
        worker.done(1, false).unwrap();
        assert_eq!(declined.read(&mut [0; 1]).unwrap(), 0);
        assert!(worker.write(1).is_err());
        let mut expired = pending(&mut worker, 2);
        worker.writes.get_mut(&2).unwrap().expires = Instant::now() - Duration::from_secs(1);
        worker.pump();
        assert_eq!(expired.read(&mut [0; 1]).unwrap(), 0);
        assert!(worker.write(2).is_err());
    }
    #[test]
    fn rejecting_completion_revokes_a_writer_already_handed_to_the_application() {
        let mut worker = state();
        let mut peer = pending(&mut worker, 4);
        let mut writer = File::from(worker.write(4).unwrap());
        worker.done(4, false).unwrap();
        assert_eq!(
            writer.write(b"late bytes").unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(peer.read(&mut [0; 1]).unwrap(), 0);
    }
}
