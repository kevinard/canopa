//! Exclusive local runtime ownership and bounded window-launch intent.
//! Filesystem work stays on the operation interfaces, never on this transport.
use futures_channel::mpsc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::{self, File},
    io::{self, Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

const PROTOCOL: u32 = 1;
const FRAME_LIMIT: usize = 65_536;
const OPENING_LIMIT: usize = 64;
const IO_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Configuration {
    Discover,
    Defaults,
    File(#[serde(with = "crate::path_serde")] PathBuf),
}

impl Configuration {
    pub fn request(&self) -> canopa_config::Request {
        match self {
            Self::Discover => canopa_config::Request::Discover,
            Self::Defaults => canopa_config::Request::Defaults,
            Self::File(path) => canopa_config::Request::common(path),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Command {
    Open(#[serde(with = "crate::path_serde::sequence")] Vec<PathBuf>),
    Resident,
    Quit,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    protocol: u32,
    configuration: Option<Configuration>,
    command: Command,
}

#[derive(Serialize, Deserialize)]
enum Reply {
    Accepted,
    Stopping,
    Refused(String),
}

pub enum Claim {
    Primary(Instance),
    Forwarded,
    Absent,
}

/// A presentation receipt: taking it does not release the accepted-launch hold.
#[derive(Debug)]
pub struct Opening {
    pub presentation: crate::presentation::Presentation,
    receipt: Arc<()>,
    serial: u64,
}

struct State {
    configuration: Configuration,
    resident: bool,
    accepting: bool,
    can_open: bool,
    serial: u64,
    openings: VecDeque<Opening>,
    pending: BTreeMap<u64, Arc<()>>,
    notice: mpsc::Sender<()>,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    stopped: AtomicBool,
}

struct Owner {
    shared: Arc<Shared>,
    notices: Mutex<Option<mpsc::Receiver<()>>>,
    thread: Option<JoinHandle<()>>,
    stop: Option<UnixStream>,
    endpoint: PathBuf,
    identity: (u64, u64),
    _lock: File,
}

/// Clones refer to the same owner. Hold this before starting history or workers.
#[derive(Clone)]
pub struct Instance(Arc<Owner>);

impl Instance {
    /// Claim this state directory, or forward to its existing owner. An absent
    /// configuration inherits the existing selection; it means Discover initially.
    pub fn launch(
        directory: &Path,
        runtime: &Path,
        mut configuration: Option<Configuration>,
        mut command: Command,
        can_open: bool,
    ) -> io::Result<Claim> {
        if let Some(Configuration::File(path)) = &mut configuration {
            *path = std::path::absolute(&*path)?;
        }
        if let Command::Open(paths) = &mut command {
            for path in paths {
                let bytes = path.as_os_str().as_bytes();
                if !bytes.starts_with(b"file://") && bytes != b"~" && !bytes.starts_with(b"~/") {
                    *path = std::path::absolute(&*path)?;
                }
            }
        }
        let request = Request {
            protocol: PROTOCOL,
            configuration,
            command,
        };
        validate(&request)?;
        let payload = serde_json::to_vec(&request).map_err(io::Error::other)?;
        if payload.len() > FRAME_LIMIT {
            return Err(io::Error::other("The Files launch request is too large."));
        }
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)?;
        let directory = fs::canonicalize(directory)?;
        if !runtime.is_absolute() {
            return Err(io::Error::other("An absolute XDG_RUNTIME_DIR is required."));
        }
        let metadata = fs::metadata(runtime)?;
        if !metadata.is_dir()
            || metadata.uid() != rustix::process::getuid().as_raw()
            || metadata.mode() & 0o077 != 0
        {
            return Err(io::Error::other(
                "XDG_RUNTIME_DIR must be an owner-only directory.",
            ));
        }
        let digest = Sha256::digest(directory.as_os_str().as_bytes());
        let name = digest[..16]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let endpoint = runtime.join(format!("canopa-files-{name}.sock"));
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
            )
            .open(directory.join("runtime.lock"))?;
        if !lock.metadata()?.is_file() {
            return Err(io::Error::other(
                "The Files runtime lock is not a regular file.",
            ));
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
                Ok(()) => break,
                Err(rustix::io::Errno::WOULDBLOCK) => {
                    match forward(&endpoint, &payload) {
                        Ok(Reply::Accepted) => return Ok(Claim::Forwarded),
                        Ok(Reply::Refused(error)) => return Err(io::Error::other(error)),
                        Ok(Reply::Stopping) => {}
                        // An owner can still be publishing its endpoint. Only
                        // connection refusal is safe to retry, never a lost reply.
                        Err(error)
                            if matches!(
                                error.kind(),
                                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                            ) => {}
                        Err(error) => return Err(error),
                    }
                    if Instant::now() >= deadline {
                        return Err(io::Error::other(
                            "The owning Files runtime is starting, stopping, or belongs to another session. Try again shortly.",
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(error) => return Err(error.into()),
            }
        }
        if matches!(request.command, Command::Open(_)) && !can_open {
            return Err(io::Error::other(
                "A display environment is required to open a Files browser.",
            ));
        }
        if matches!(request.command, Command::Quit) {
            return Ok(Claim::Absent);
        }
        match fs::symlink_metadata(&endpoint) {
            Ok(metadata)
                if metadata.file_type().is_socket()
                    && metadata.uid() == rustix::process::getuid().as_raw() =>
            {
                match connect_endpoint(&endpoint, Instant::now() + IO_TIMEOUT) {
                    Ok(_) => {
                        return Err(io::Error::other(
                            "A live Files activation endpoint already exists.",
                        ));
                    }
                    Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                        fs::remove_file(&endpoint)?
                    }
                    Err(error) => return Err(error),
                }
            }
            Ok(_) => {
                return Err(io::Error::other(
                    "Refusing to replace a non-socket Files activation path.",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let listener = UnixListener::bind(&endpoint)?;
        listener.set_nonblocking(true)?;
        let (stop, wake) = UnixStream::pair()?;
        fs::set_permissions(&endpoint, fs::Permissions::from_mode(0o600))?;
        let metadata = fs::symlink_metadata(&endpoint)?;
        let (notice, notices) = mpsc::channel(0);
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                configuration: request
                    .configuration
                    .clone()
                    .unwrap_or(Configuration::Discover),
                resident: false,
                accepting: true,
                can_open,
                serial: 0,
                openings: VecDeque::new(),
                pending: BTreeMap::new(),
                notice,
            }),
            changed: Condvar::new(),
            stopped: AtomicBool::new(false),
        });
        accept(&shared, request);
        let serving = shared.clone();
        let thread = std::thread::Builder::new()
            .name("files-activation".into())
            .spawn(move || {
                if let Err(error) = serve(listener, wake, &serving) {
                    eprintln!("canopa-files: activation stopped: {error}");
                    let mut state = serving.state.lock().unwrap_or_else(|e| e.into_inner());
                    state.resident = false;
                    state.accepting = false;
                    let _ = state.notice.try_send(());
                    serving.changed.notify_all();
                }
            })?;
        Ok(Claim::Primary(Self(Arc::new(Owner {
            shared,
            notices: Mutex::new(Some(notices)),
            thread: Some(thread),
            stop: Some(stop),
            endpoint,
            identity: (metadata.dev(), metadata.ino()),
            _lock: lock,
        }))))
    }

    pub fn configuration(&self) -> Configuration {
        self.0
            .shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .configuration
            .clone()
    }

    pub fn take_openings(&self) -> Vec<Opening> {
        self.0
            .shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .openings
            .drain(..)
            .collect()
    }

    /// Admit a local presentation through the same bounded shutdown/launch gate.
    pub fn present(&self, presentation: crate::presentation::Presentation) -> io::Result<()> {
        presentation.validate()?;
        let payload = serde_json::to_vec(&presentation).map_err(io::Error::other)?;
        if payload.len() > FRAME_LIMIT {
            return Err(io::Error::other("The Files launch request is too large."));
        }
        match admit(&self.0.shared, None, Intent::Present(presentation)) {
            Reply::Accepted => Ok(()),
            Reply::Stopping => Err(io::Error::other("Files is stopping; try again shortly.")),
            Reply::Refused(message) => Err(io::Error::other(message)),
        }
    }

    pub fn presented(&self, opening: Opening) {
        let mut state = self
            .0
            .shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if state
            .pending
            .get(&opening.serial)
            .is_some_and(|receipt| Arc::ptr_eq(receipt, &opening.receipt))
        {
            state.pending.remove(&opening.serial);
        }
    }

    /// Called only after all other application holds are clear. Closing admission
    /// is atomic with checking accepted requests and resident policy.
    pub fn try_exit(&self) -> bool {
        let mut state = self
            .0
            .shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if state.resident || !state.pending.is_empty() {
            return false;
        }
        state.accepting = false;
        true
    }

    /// Before Iced: block while resident. Only a requested drain has a timeout,
    /// allowing the host to observe its remaining active backend holds.
    pub fn wait_for_open(&self) -> bool {
        let shared = &self.0.shared;
        let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        while state.openings.is_empty() && state.resident && state.accepting {
            state = shared
                .changed
                .wait(state)
                .unwrap_or_else(|e| e.into_inner());
        }
        if state.openings.is_empty() && !state.resident {
            let (next, _) = shared
                .changed
                .wait_timeout(state, Duration::from_millis(250))
                .unwrap_or_else(|e| e.into_inner());
            state = next;
        }
        !state.openings.is_empty()
    }

    pub fn take_notices(&self) -> Option<mpsc::Receiver<()>> {
        self.0
            .notices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }
}

fn validate(request: &Request) -> io::Result<()> {
    if request.protocol != PROTOCOL {
        return Err(io::Error::other(
            "The running Files activation protocol is incompatible. Close its windows and finish its work before upgrading.",
        ));
    }
    if matches!(&request.command, Command::Open(paths) if paths.len() > OPENING_LIMIT || paths.iter().any(|p| p.as_os_str().as_bytes().contains(&0)))
    {
        return Err(io::Error::other(
            "A Files launch supports at most 64 locations without NUL bytes.",
        ));
    }
    Ok(())
}

fn accept(shared: &Shared, request: Request) -> Reply {
    if let Err(error) = validate(&request) {
        return Reply::Refused(error.to_string());
    }
    let intent = match request.command {
        Command::Open(paths) => Intent::Present(crate::presentation::Presentation::open(paths)),
        Command::Resident => Intent::Resident,
        Command::Quit => Intent::Quit,
    };
    admit(shared, request.configuration, intent)
}

enum Intent {
    Present(crate::presentation::Presentation),
    Resident,
    Quit,
}

fn admit(shared: &Shared, configuration: Option<Configuration>, intent: Intent) -> Reply {
    let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
    if !state.accepting {
        return Reply::Stopping;
    }
    if configuration
        .as_ref()
        .is_some_and(|selection| selection != &state.configuration)
    {
        return Reply::Refused("Files is already running with a different configuration selection. Finish its work and close it before changing configuration.".into());
    }
    match intent {
        Intent::Present(presentation) => {
            if !state.can_open {
                return Reply::Refused("This Files owner started without a display environment. Use --quit after finishing its work, then start Files in the graphical session.".into());
            }
            if state.pending.len() >= OPENING_LIMIT {
                return Reply::Refused(
                    "Files has too many pending window requests. Try again shortly.".into(),
                );
            }
            let Some(serial) = state.serial.checked_add(1) else {
                return Reply::Refused(
                    "Files launch identities are exhausted. Restart after finishing its work."
                        .into(),
                );
            };
            state.serial = serial;
            let receipt = Arc::new(());
            state.pending.insert(serial, receipt.clone());
            state.openings.push_back(Opening {
                presentation,
                receipt,
                serial,
            });
        }
        Intent::Resident => state.resident = true,
        Intent::Quit => state.resident = false,
    }
    let _ = state.notice.try_send(());
    shared.changed.notify_all();
    Reply::Accepted
}

fn serve(listener: UnixListener, wake: UnixStream, shared: &Shared) -> io::Result<()> {
    use rustix::event::{PollFd, PollFlags, poll};
    loop {
        let mut fds = [
            PollFd::new(&listener, PollFlags::IN),
            PollFd::new(&wake, PollFlags::IN),
        ];
        match poll(&mut fds, None) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
        }
        if shared.stopped.load(Ordering::Acquire) || !fds[1].revents().is_empty() {
            return Ok(());
        }
        let mut connection = match listener.accept() {
            Ok((connection, _)) => connection,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                continue;
            }
            Err(error) => return Err(error),
        };
        connection.set_nonblocking(false)?;
        let deadline = Instant::now() + IO_TIMEOUT;
        let result = read_frame(&mut connection, deadline).and_then(|bytes| {
            let request = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
            Ok(accept(shared, request))
        });
        let reply = result.unwrap_or_else(|error| Reply::Refused(error.to_string()));
        if let Ok(bytes) = serde_json::to_vec(&reply) {
            let _ = write_frame(&mut connection, &bytes, deadline);
        }
    }
}

fn connect_endpoint(endpoint: &Path, deadline: Instant) -> io::Result<UnixStream> {
    use rustix::net::{
        AddressFamily, SocketAddrUnix, SocketFlags, SocketType, connect, socket_with,
    };
    let address = SocketAddrUnix::new(endpoint)?;
    loop {
        let fd = socket_with(
            AddressFamily::UNIX,
            SocketType::STREAM,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )?;
        match connect(&fd, &address) {
            Ok(()) => {
                let stream = UnixStream::from(fd);
                stream.set_nonblocking(false)?;
                return Ok(stream);
            }
            Err(
                rustix::io::Errno::AGAIN | rustix::io::Errno::INPROGRESS | rustix::io::Errno::INTR,
            ) => {
                std::thread::sleep(remaining(deadline)?.min(Duration::from_millis(20)));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn forward(endpoint: &Path, payload: &[u8]) -> io::Result<Reply> {
    let deadline = Instant::now() + IO_TIMEOUT;
    let mut stream = connect_endpoint(endpoint, deadline)?;
    write_frame(&mut stream, payload, deadline)?;
    serde_json::from_slice(&read_frame(&mut stream, deadline)?).map_err(io::Error::other)
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                "Files activation timed out; the request may have been accepted.",
            )
        })
}

fn read_exact(stream: &mut UnixStream, mut output: &mut [u8], deadline: Instant) -> io::Result<()> {
    while !output.is_empty() {
        stream.set_read_timeout(Some(remaining(deadline)?))?;
        let count = stream.read(output)?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Files activation closed before acknowledgement; the request may have been accepted.",
            ));
        }
        output = &mut output[count..];
    }
    Ok(())
}

fn read_frame(stream: &mut UnixStream, deadline: Instant) -> io::Result<Vec<u8>> {
    let mut length = [0; 4];
    read_exact(stream, &mut length, deadline)?;
    let length = u32::from_be_bytes(length) as usize;
    if length > FRAME_LIMIT {
        return Err(io::Error::other("Files activation frame is too large."));
    }
    let mut bytes = vec![0; length];
    read_exact(stream, &mut bytes, deadline)?;
    Ok(bytes)
}

fn write_frame(stream: &mut UnixStream, bytes: &[u8], deadline: Instant) -> io::Result<()> {
    stream.set_write_timeout(Some(remaining(deadline)?))?;
    stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stream.set_write_timeout(Some(remaining(deadline)?))?;
    stream.write_all(bytes)
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.shared.stopped.store(true, Ordering::Release);
        // Closing the private peer wakes poll even if the public endpoint was
        // removed, replaced, or has a saturated accept backlog.
        self.stop.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        if let Ok(metadata) = fs::symlink_metadata(&self.endpoint)
            && (metadata.dev(), metadata.ino()) == self.identity
        {
            let _ = fs::remove_file(&self.endpoint);
        }
    }
}
