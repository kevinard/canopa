use super::{
    pipes::{self, Payload},
    *,
};
use crate::{
    file_clipboard::{GNOME, KDE, LIMIT, URIS},
    transfers::TransferKind,
};
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    net,
};
use std::{collections::BTreeMap, fs::File, io::Read, os::fd::AsFd, path::PathBuf, time::Instant};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    protocol::{wl_callback, wl_registry, wl_seat},
};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1 as device, ext_data_control_manager_v1 as manager,
    ext_data_control_offer_v1 as offer, ext_data_control_source_v1 as source,
};

fn changed() -> String {
    fl!("files-clipboard-changed")
}

fn error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

fn connection() -> io::Result<Connection> {
    // This is a second connection. Never consume or mutate WAYLAND_SOCKET,
    // which may belong to the application's Iced connection.
    let display = std::env::var_os("WAYLAND_DISPLAY")
        .map(PathBuf::from)
        .ok_or_else(|| error("The file clipboard needs a named Wayland display"))?;
    let path = if display.is_absolute() {
        display
    } else {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(|| {
                error("The file clipboard needs an absolute Wayland runtime directory")
            })?;
        runtime.join(display)
    };
    let socket = net::socket_with(
        net::AddressFamily::UNIX,
        net::SocketType::STREAM,
        net::SocketFlags::NONBLOCK | net::SocketFlags::CLOEXEC,
        None,
    )?;
    // Linux Unix sockets connect immediately or fail (including a full listen
    // queue); NONBLOCK prevents startup from waiting on the compositor.
    net::connect(&socket, &net::SocketAddrUnix::new(&path)?)?;
    Connection::from_socket(UnixStream::from(socket)).map_err(error)
}

enum Callback {
    Registry,
    Device,
    Read(u64),
}

#[derive(Default)]
struct Types {
    formats: u8,
    owned: Option<Intent>,
}
impl Types {
    fn flag(mime: &str) -> u8 {
        match mime {
            GNOME => 1,
            URIS => 2,
            KDE => 4,
            _ => 0,
        }
    }
    fn has(&self, mime: &str) -> bool {
        self.formats & Self::flag(mime) != 0
    }
    fn insert(&mut self, mime: &str) {
        self.formats |= Self::flag(mime);
    }
}

struct SourceData {
    intent: Intent,
    marker: String,
    payload: Mutex<Arc<Payload>>,
}
#[derive(Clone)]
struct Owned {
    proxy: source::ExtDataControlSourceV1,
    data: Arc<SourceData>,
}
struct Publication {
    source: Owned,
    reply: Reply<Intent>,
    deadline: Instant,
}

struct Worker {
    shared: Arc<Shared>,
    wake: UnixStream,
    manager: Option<manager::ExtDataControlManagerV1>,
    manager_name: Option<u32>,
    seat: Option<wl_seat::WlSeat>,
    seat_name: Option<u32>,
    device: Option<device::ExtDataControlDeviceV1>,
    offer: Option<offer::ExtDataControlOfferV1>,
    source: Option<Owned>,
    publications: BTreeMap<Intent, Publication>,
    reads: BTreeMap<u64, pipes::Receive>,
    writes: Vec<pipes::Send>,
    nonce: [u8; 16],
    next: u64,
    revision: u64,
    ready: bool,
    initialization_deadline: Option<Instant>,
    stop: bool,
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
    fn owned(&self) -> Option<Intent> {
        let active = self.source.as_ref()?;
        let types = self
            .offer
            .as_ref()?
            .data::<Mutex<Types>>()?
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        (active.proxy.is_alive() && types.owned == Some(active.data.intent))
            .then_some(active.data.intent)
    }
    fn changed(&mut self) {
        if let Some(next) = self.revision.checked_add(1) {
            self.revision = next;
        } else {
            self.failure = Some("Clipboard revisions exhausted".into());
        }
        self.snapshot();
        for (_, read) in std::mem::take(&mut self.reads) {
            let _ = read.reply.send(Err(changed()));
        }
    }
    fn snapshot(&self) {
        let has_files = self
            .offer
            .as_ref()
            .and_then(|offer| offer.data::<Mutex<Types>>())
            .is_some_and(|types| {
                let types = types.lock().unwrap_or_else(|e| e.into_inner());
                types.has(GNOME) || types.has(URIS)
            });
        self.shared.set(Snapshot {
            ready: self.ready,
            revision: self.revision,
            owned: self.owned(),
            has_files,
            error: self.failure.clone(),
        });
    }
    fn publish(&mut self, selection: Selection, reply: Reply<Intent>, qh: &QueueHandle<Self>) {
        let prepared = (|| {
            if self.publications.len() >= 8 {
                return Err(error(fl!("files-clipboard-busy")));
            }
            let payload = Payload::new(selection)?;
            let intent = Intent {
                nonce: self.nonce,
                sequence: self.next()?,
            };
            let nonce: String = self
                .nonce
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            let data = Arc::new(SourceData {
                intent,
                marker: format!("application/x-canopa-files-{nonce}-{}", intent.sequence),
                payload: Mutex::new(payload),
            });
            let proxy = self
                .manager
                .as_ref()
                .unwrap()
                .create_data_source(qh, data.clone());
            for mime in [GNOME, URIS, KDE, &data.marker] {
                proxy.offer(mime.to_owned());
            }
            Ok(Owned { proxy, data })
        })();
        match prepared {
            Ok(source) => {
                self.device
                    .as_ref()
                    .unwrap()
                    .set_selection(Some(&source.proxy));
                self.publications.insert(
                    source.data.intent,
                    Publication {
                        source: source.clone(),
                        reply,
                        deadline: Instant::now() + pipes::DEADLINE,
                    },
                );
                self.source = Some(source);
                self.changed();
            }
            Err(error) => {
                let _ = reply.send(Err(error.to_string()));
            }
        }
    }
    fn read(&mut self, reply: Reply<Received>) {
        if self.reads.len() >= 4 || !self.publications.is_empty() {
            let _ = reply.send(Err(fl!("files-clipboard-busy-paste")));
            return;
        }
        let prepared = (|| {
            let offer = self
                .offer
                .as_ref()
                .ok_or_else(|| error("The clipboard contains no files"))?;
            let types = offer
                .data::<Mutex<Types>>()
                .unwrap()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let mime = if types.has(GNOME) {
                GNOME
            } else if types.has(URIS) {
                URIS
            } else {
                return Err(error("The clipboard contains no files"));
            };
            let kde = mime == URIS && types.has(KDE);
            drop(types);
            let (part, pipe) = pipes::Part::new(LIMIT)?;
            let mut parts = vec![part];
            let mut requests = vec![(mime, pipe)];
            if kde {
                let (part, pipe) = pipes::Part::new(1)?;
                parts.push(part);
                requests.push((KDE, pipe));
            }
            for (mime, pipe) in requests {
                offer.receive(mime.to_owned(), pipe.as_fd());
            }
            Ok((mime, parts))
        })();
        match prepared {
            Ok((mime, parts)) => match self.next() {
                Ok(id) => {
                    self.reads.insert(
                        id,
                        pipes::Receive {
                            revision: self.revision,
                            owned: self.owned(),
                            parts,
                            decoded: None,
                            mime,
                            reply,
                            deadline: Instant::now() + pipes::DEADLINE,
                        },
                    );
                }
                Err(error) => {
                    let _ = reply.send(Err(error.to_string()));
                }
            },
            Err(error) => {
                let _ = reply.send(Err(error.to_string()));
            }
        }
    }
    fn replace_cut(&mut self, intent: Intent, selection: Option<Selection>, reply: Reply<bool>) {
        let result = (|| {
            let Some(source) = self
                .source
                .clone()
                .filter(|source| self.owned() == Some(intent) && source.data.intent == intent)
            else {
                return Ok(false);
            };
            let current = source
                .data
                .payload
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            if current.selection.kind() != TransferKind::Move {
                return Ok(false);
            }
            let remaining = if let Some(selection) = selection {
                if selection.kind() != TransferKind::Move {
                    return Err(error("Remaining Cut files must retain the Move action"));
                }
                let proposed: std::collections::BTreeSet<_> = selection.paths().iter().collect();
                let paths: Vec<_> = current
                    .selection
                    .paths()
                    .iter()
                    .filter(|path| proposed.contains(path))
                    .cloned()
                    .collect();
                if paths.as_slice() == current.selection.paths() {
                    return Ok(true);
                }
                if paths.is_empty() {
                    None
                } else {
                    Some(Selection::new(paths, TransferKind::Move)?)
                }
            } else {
                None
            };
            if let Some(remaining) = remaining {
                let payload = Payload::new(remaining)?;
                *source
                    .data
                    .payload
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()) = payload;
            } else {
                // Destroying this particular source cannot clear a replacement
                // belonging to another client, even if cancellation is in flight.
                if source.proxy.is_alive() {
                    source.proxy.destroy();
                }
                self.source = None;
                if let Some(offer) = self.offer.take() {
                    offer.destroy();
                }
            }
            self.changed();
            Ok(true)
        })();
        let _ = reply.send(result.map_err(|error: io::Error| error.to_string()));
    }
    fn pump(&mut self, conn: &Connection, qh: &QueueHandle<Self>) {
        let mut failures = Vec::new();
        for (id, read) in &mut self.reads {
            match read.pump() {
                Ok(true) => {
                    conn.display().sync(qh, Callback::Read(*id));
                }
                Ok(false) => {}
                Err(error) => failures.push((*id, error.to_string())),
            }
        }
        for (id, reason) in failures {
            if let Some(read) = self.reads.remove(&id) {
                let _ = read.reply.send(Err(reason));
            }
        }
        self.writes
            .retain_mut(|write| Instant::now() < write.deadline && !write.pump());
    }
    fn expire(&mut self) {
        let now = Instant::now();
        let reads: Vec<_> = self
            .reads
            .iter()
            .filter(|(_, read)| now >= read.deadline)
            .map(|(id, _)| *id)
            .collect();
        for id in reads {
            if let Some(read) = self.reads.remove(&id) {
                let _ = read.reply.send(Err(fl!("files-clipboard-owner-silent")));
            }
        }
        let publications: Vec<_> = self
            .publications
            .iter()
            .filter(|(_, publication)| now >= publication.deadline)
            .map(|(id, _)| *id)
            .collect();
        for id in publications {
            if let Some(publication) = self.publications.remove(&id) {
                if publication.source.proxy.is_alive() {
                    publication.source.proxy.destroy();
                }
                if self
                    .source
                    .as_ref()
                    .is_some_and(|source| source.data.intent == id)
                {
                    self.source = None;
                    self.changed();
                }
                self.snapshot();
                let _ = publication
                    .reply
                    .send(Err(fl!("files-clipboard-not-accepted-compositor")));
            }
        }
    }
    fn deadline(&self) -> Option<Timespec> {
        self.reads
            .values()
            .map(|read| read.deadline)
            .chain(self.writes.iter().map(|write| write.deadline))
            .chain(
                self.publications
                    .values()
                    .map(|publication| publication.deadline),
            )
            .chain(self.initialization_deadline)
            .min()
            .map(|deadline| {
                Timespec::try_from(deadline.saturating_duration_since(Instant::now())).unwrap()
            })
    }
}

pub(super) fn run(
    receiver: &mpsc::Receiver<Control>,
    wake: UnixStream,
    shared: Arc<Shared>,
) -> io::Result<()> {
    let conn = connection()?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let mut nonce = [0u8; 16];
    if rustix::rand::getrandom(&mut nonce[..], rustix::rand::GetRandomFlags::empty())?
        != nonce.len()
    {
        return Err(error("Could not prepare a fresh clipboard identity"));
    }
    let mut state = Worker {
        shared,
        wake,
        manager: None,
        manager_name: None,
        seat: None,
        seat_name: None,
        device: None,
        offer: None,
        source: None,
        publications: BTreeMap::new(),
        reads: BTreeMap::new(),
        writes: Vec::new(),
        nonce,
        next: 0,
        revision: 0,
        ready: false,
        initialization_deadline: Some(Instant::now() + pipes::DEADLINE),
        stop: false,
        failure: None,
    };
    let _registry = conn.display().get_registry(&qh, ());
    conn.display().sync(&qh, Callback::Registry);
    let result = dispatch_loop(receiver, &conn, &mut queue, &qh, &mut state);
    state.ready = false;
    state.source = None;
    state.offer = None;
    state.failure = result.as_ref().err().map(ToString::to_string);
    state.snapshot();
    let reason = state
        .failure
        .clone()
        .unwrap_or_else(|| "The file clipboard stopped".into());
    for (_, publication) in std::mem::take(&mut state.publications) {
        let _ = publication.reply.send(Err(reason.clone()));
    }
    for (_, read) in std::mem::take(&mut state.reads) {
        let _ = read.reply.send(Err(reason.clone()));
    }
    result
}

fn dispatch_loop(
    receiver: &mpsc::Receiver<Control>,
    conn: &Connection,
    queue: &mut EventQueue<Worker>,
    qh: &QueueHandle<Worker>,
    state: &mut Worker,
) -> io::Result<()> {
    loop {
        queue.dispatch_pending(state).map_err(error)?;
        if let Some(failure) = state.failure.take() {
            return Err(error(failure));
        }
        if state
            .initialization_deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(error(fl!("files-clipboard-compositor-silent")));
        }
        for _ in 0..8 {
            match receiver.try_recv() {
                Ok(Control::Stop) | Err(mpsc::TryRecvError::Disconnected) => {
                    state.stop = true;
                    break;
                }
                Ok(control) if !state.ready => control.reject(&fl!("files-clipboard-starting")),
                Ok(Control::Publish(selection, reply)) => state.publish(selection, reply, qh),
                Ok(Control::Read(reply)) => state.read(reply),
                Ok(Control::ReplaceCut(intent, selection, reply)) => {
                    state.replace_cut(intent, selection, reply)
                }
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        if state.stop {
            return Ok(());
        }
        state.pump(conn, qh);
        state.expire();
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
        let mut flags = PollFlags::IN;
        if write_pending {
            flags |= PollFlags::OUT;
        }
        let mut fds = vec![
            PollFd::new(&*queue, flags),
            PollFd::new(&state.wake, PollFlags::IN),
        ];
        for read in state.reads.values() {
            for file in read.parts.iter().filter_map(|part| part.file.as_ref()) {
                fds.push(PollFd::new(file, PollFlags::IN));
            }
        }
        for write in &state.writes {
            fds.push(PollFd::new(&write.file, PollFlags::OUT));
        }
        let timeout = state.deadline();
        match poll(&mut fds, timeout.as_ref()) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => {
                drop(guard);
                continue;
            }
            Err(failure) => return Err(failure.into()),
        }
        let readable = fds[0]
            .revents()
            .intersects(PollFlags::IN | PollFlags::ERR | PollFlags::HUP);
        drop(fds);
        if readable {
            if let Err(failure) = guard.read() {
                match failure {
                    wayland_client::backend::WaylandError::Io(ref error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    failure => return Err(error(failure)),
                }
            }
        } else {
            drop(guard);
        }
        let mut buffer = [0u8; 64];
        while matches!((&state.wake).read(&mut buffer), Ok(count) if count > 0) {}
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
                    state.failure =
                        Some("The file clipboard requires an unambiguous Wayland seat".into());
                } else {
                    state.seat = Some(registry.bind(name, 1, qh, ()));
                    state.seat_name = Some(name);
                }
            }
            wl_registry::Event::GlobalRemove { name }
                if state.manager_name == Some(name) || state.seat_name == Some(name) =>
            {
                state.failure =
                    Some("The clipboard's original Wayland seat or manager disappeared".into())
            }
            _ => {}
        }
    }
}
wayland_client::delegate_noop!(Worker: ignore manager::ExtDataControlManagerV1);
wayland_client::delegate_noop!(Worker: ignore wl_seat::WlSeat);

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
                state.changed();
                if let Some(intent) = state.owned()
                    && let Some(publication) = state.publications.remove(&intent)
                {
                    let _ = publication.reply.send(Ok(intent));
                }
            }
            device::Event::PrimarySelection { id: Some(offer) } => offer.destroy(),
            device::Event::Finished => state.failure = Some(fl!("files-clipboard-disconnected")),
            _ => {}
        }
    }
    wayland_client::event_created_child!(Worker, device::ExtDataControlDeviceV1, [0 => (offer::ExtDataControlOfferV1, Mutex::new(Types::default()))]);
}
impl Dispatch<offer::ExtDataControlOfferV1, Mutex<Types>> for Worker {
    fn event(
        state: &mut Self,
        _: &offer::ExtDataControlOfferV1,
        event: offer::Event,
        types: &Mutex<Types>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let offer::Event::Offer { mime_type } = event {
            let mut types = types.lock().unwrap_or_else(|e| e.into_inner());
            types.insert(&mime_type);
            if let Some(source) = &state.source
                && mime_type == source.data.marker
            {
                types.owned = Some(source.data.intent);
            }
        }
    }
}
impl Dispatch<source::ExtDataControlSourceV1, Arc<SourceData>> for Worker {
    fn event(
        state: &mut Self,
        source: &source::ExtDataControlSourceV1,
        event: source::Event,
        data: &Arc<SourceData>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            source::Event::Send { mime_type, fd } => {
                if state.writes.len() >= 8 {
                    return;
                }
                let payload = data
                    .payload
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let bytes = if mime_type == data.marker {
                    Some(Arc::from([]))
                } else {
                    payload.bytes(&mime_type)
                };
                if let Some(bytes) = bytes
                    && let Ok(write) = pipes::Send::new(File::from(fd), bytes)
                {
                    state.writes.push(write);
                }
            }
            source::Event::Cancelled => {
                if source.is_alive() {
                    source.destroy();
                }
                let publication = state.publications.remove(&data.intent);
                if state
                    .source
                    .as_ref()
                    .is_some_and(|source| source.data.intent == data.intent)
                {
                    state.source = None;
                    state.changed();
                }
                if let Some(publication) = publication {
                    let _ = publication.reply.send(Err(fl!("files-clipboard-replaced")));
                }
            }
            _ => {}
        }
    }
}
impl Dispatch<wl_callback::WlCallback, Callback> for Worker {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        _: wl_callback::Event,
        callback: &Callback,
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let id = match callback {
            Callback::Registry => {
                match (&state.manager, &state.seat) {
                    (Some(manager), Some(seat)) => {
                        state.device = Some(manager.get_data_device(seat, qh, ()));
                        conn.display().sync(qh, Callback::Device);
                    }
                    (None, _) => state.failure = Some(fl!("files-clipboard-unsupported")),
                    (_, None) => state.failure = Some(fl!("files-clipboard-no-seat")),
                }
                return;
            }
            Callback::Device => {
                state.initialization_deadline = None;
                state.ready = true;
                state.snapshot();
                return;
            }
            Callback::Read(id) => id,
        };
        if let Some(read) = state.reads.remove(id) {
            let result = if read.revision != state.revision {
                Err(changed())
            } else if let Some(mut selection) = read.decoded {
                let owned = state.owned().filter(|intent| Some(*intent) == read.owned);
                if owned.is_some()
                    && let Some(source) = &state.source
                {
                    selection = source
                        .data
                        .payload
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .selection
                        .clone();
                }
                Ok(Received { selection, owned })
            } else {
                Err(fl!("files-clipboard-read-unfinished"))
            };
            let _ = read.reply.send(result);
        }
    }
}
