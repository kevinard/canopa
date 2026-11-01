//! Session-specific logind requests and compositor-confirmed locked hints.
use anyhow::{Context, Result, bail, ensure};
use futures_util::StreamExt;
use std::{
    os::unix::{fs::OpenOptionsExt, process::CommandExt},
    path::PathBuf,
    time::Duration,
};
use tokio::sync::mpsc;
use zbus::{Connection, Proxy, zvariant::OwnedObjectPath};
const SERVICE: &str = "org.freedesktop.login1";
const MANAGER: &str = "/org/freedesktop/login1";
const SESSION: &str = "org.freedesktop.login1.Session";
const DEADLINE: Duration = Duration::from_secs(2);
#[derive(Clone)]
pub struct Target {
    connection: Connection,
    pub path: OwnedObjectPath,
    pub id: String,
}
impl Target {
    async fn proxy(&self) -> Result<Proxy<'_>> {
        Ok(tokio::time::timeout(
            DEADLINE,
            zbus::proxy::Builder::new(&self.connection)
                .destination(SERVICE)?
                .path(self.path.as_str())?
                .interface(SESSION)?
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build(),
        )
        .await
        .context("logind proxy timed out")??)
    }
    pub async fn set_locked(&self, locked: bool) -> Result<()> {
        tokio::time::timeout(DEADLINE, async {
            self.proxy()
                .await?
                .call::<_, _, ()>("SetLockedHint", &(locked,))
                .await?;
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("logind hint timed out")??;
        Ok(())
    }
}
/// Discovery works from user services too. Never choose an arbitrary session when
/// the same uid has several graphical logins; --session/XDG_SESSION_ID disambiguates.
pub async fn discover(connection: Connection, selected: Option<&str>) -> Result<Target> {
    let manager = Proxy::new(
        &connection,
        SERVICE,
        MANAGER,
        "org.freedesktop.login1.Manager",
    )
    .await?;
    let explicit = selected.map(str::to_owned).or_else(|| {
        std::env::var("XDG_SESSION_ID")
            .ok()
            .filter(|v| !v.is_empty())
    });
    let path: OwnedObjectPath = if let Some(id) = explicit {
        manager.call("GetSession", &(id,)).await?
    } else if let Ok(path) = manager
        .call::<_, _, OwnedObjectPath>("GetSessionByPID", &(std::process::id(),))
        .await
    {
        path
    } else {
        let sessions: Vec<(String, u32, String, String, OwnedObjectPath)> =
            manager.call("ListSessions", &()).await?;
        let mut candidates = Vec::new();
        for (_, uid, _, _, path) in sessions {
            // SAFETY: getuid has no preconditions.
            if uid != unsafe { libc::getuid() } {
                continue;
            }
            let proxy = Proxy::new(&connection, SERVICE, path.as_str(), SESSION).await?;
            let kind: String = proxy.get_property("Type").await?;
            let class: String = proxy.get_property("Class").await?;
            if kind == "wayland" && class == "user" {
                candidates.push(path);
            }
        }
        ensure!(
            candidates.len() == 1,
            "cannot select a unique graphical logind session; set --session or XDG_SESSION_ID"
        );
        candidates.remove(0)
    };
    let proxy = Proxy::new(&connection, SERVICE, path.as_str(), SESSION).await?;
    let (uid, _): (u32, OwnedObjectPath) = proxy.get_property("User").await?;
    ensure!(
        uid == unsafe { libc::getuid() },
        "logind session belongs to another user"
    );
    let kind: String = proxy.get_property("Type").await?;
    ensure!(kind == "wayland", "logind session is not a Wayland login");
    let id: String = proxy.get_property("Id").await?;
    drop(proxy);
    Ok(Target {
        connection,
        path,
        id,
    })
}
async fn connect(selected: Option<&str>) -> Result<Target> {
    tokio::time::timeout(DEADLINE, async {
        discover(Connection::system().await?, selected).await
    })
    .await
    .context("logind discovery timed out")?
}
struct Report {
    locked: bool,
    ack: std::sync::mpsc::SyncSender<bool>,
}
/// The status connection has its own event-driven executor. RPC acknowledgement is
/// bounded and drained before normal locker exit; dropped/crashed lockers never
/// manufacture an unlocked hint. Reconnect restores the last confirmed state.
pub struct Tracker {
    sender: Option<mpsc::UnboundedSender<Report>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Tracker {
    pub fn start(selected: Option<String>) -> Self {
        let (sender, mut receiver) = mpsc::unbounded_channel::<Report>();
        let thread = std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            runtime.block_on(async move {
                let Some(first) = receiver.recv().await else {
                    return;
                };
                let mut locked = first.locked;
                let mut ack = Some(first.ack);
                loop {
                    if let Ok(target) = connect(selected.as_deref()).await
                        && let Ok(proxy) = target.proxy().await
                        && let Ok(Ok(mut owners)) =
                            tokio::time::timeout(DEADLINE, proxy.receive_owner_changed()).await
                    {
                        loop {
                            let success = target.set_locked(locked).await.is_ok();
                            if let Some(ack) = ack.take() {
                                let _ = ack.send(success);
                            }
                            if !success {
                                break;
                            }
                            tokio::select! {
                                next = receiver.recv() => match next {
                                    Some(next) => { locked = next.locked; ack = Some(next.ack); }
                                    None => return,
                                },
                                _ = owners.next() => break,
                            }
                        }
                    }
                    if let Some(ack) = ack.take() {
                        let _ = ack.send(false);
                    }
                    // All connection/subscription failures share the same backoff.
                    // A stopped service must never turn this thread into a busy loop.
                    tokio::select! {
                        next = receiver.recv() => match next {
                            Some(next) => { locked = next.locked; ack = Some(next.ack); }
                            None => return,
                        },
                        _ = tokio::time::sleep(Duration::from_secs(1)) => {},
                    }
                }
            });
        });
        Self {
            sender: Some(sender),
            thread: Some(thread),
        }
    }
    pub fn set_locked(&self, locked: bool) {
        let (ack, reply) = std::sync::mpsc::sync_channel(1);
        if self
            .sender
            .as_ref()
            .is_none_or(|sender| sender.send(Report { locked, ack }).is_err())
            || reply.recv_timeout(Duration::from_millis(2200)) != Ok(true)
        {
            eprintln!(
                "logind locked-state reporting is unavailable; compositor lock remains authoritative"
            );
        }
    }
}
impl Drop for Tracker {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn daemon_guard(id: &str) -> Result<std::fs::File> {
    use std::os::{fd::AsRawFd, unix::fs::MetadataExt};
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .context("XDG_RUNTIME_DIR is required")?;
    let meta = std::fs::symlink_metadata(&runtime)?;
    ensure!(
        runtime.is_absolute()
            && meta.is_dir()
            && meta.uid() == unsafe { libc::getuid() }
            && meta.mode() & 0o077 == 0,
        "private XDG_RUNTIME_DIR is required"
    );
    let key = canopa_config::fingerprint(id);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(runtime.join(format!("canopa-lock-listener-{}.lock", &key[..16])))?;
    ensure!(
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
        "lock listener already running for this session"
    );
    Ok(file)
}
fn spawn_lock(
    request: &canopa_config::Request,
    session: &str,
) -> Result<(tokio::process::Child, tokio::net::UnixStream)> {
    use std::os::fd::AsRawFd;
    let (parent, child) = std::os::unix::net::UnixStream::pair()?;
    parent.set_nonblocking(true)?;
    let fd = child.as_raw_fd();
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command
        .env_remove("NOTIFY_SOCKET")
        .arg("--session")
        .arg(session)
        .arg("--ready-fd")
        .arg(fd.to_string())
        .process_group(0);
    match request {
        canopa_config::Request::Defaults => {
            command.arg("--defaults");
        }
        canopa_config::Request::File {
            path,
            format: canopa_config::Format::CanopaV1,
        } => {
            command.arg("--canopa-config").arg(path);
        }
        canopa_config::Request::Discover => {}
        _ => bail!("locker daemon requires shared configuration"),
    }
    // SAFETY: fcntl is async-signal-safe; only this readiness descriptor loses CLOEXEC.
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child_process = tokio::process::Command::from(command).spawn()?;
    drop(child);
    Ok((child_process, tokio::net::UnixStream::from_std(parent)?))
}
fn launch(
    request: &canopa_config::Request,
    session: &str,
    child: &mut Option<tokio::process::Child>,
    ready: &mut Option<tokio::net::UnixStream>,
) {
    match spawn_lock(request, session) {
        Ok((process, readiness)) => {
            *child = Some(process);
            *ready = Some(readiness);
        }
        Err(error) => eprintln!("cannot launch locker: {error}"),
    }
}
pub fn daemon(request: canopa_config::Request, selected: Option<String>) -> Result<()> {
    crate::settings::Settings::load(&request, canopa_config::Component::Lockscreen)?;
    std::fs::metadata("/etc/pam.d/canopa-lock")
        .context("install canopa-lock PAM service before starting its listener")?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async move {
            let session = Some(connect(selected.as_deref()).await?.id);
            let _guard = daemon_guard(session.as_ref().unwrap())?;
            let mut child: Option<tokio::process::Child> = None;
            let mut ready: Option<tokio::net::UnixStream> = None;
            let mut locked: Option<bool> = None;
            let mut notified = false;
            let mut pending = false;
            loop {
                let target = match connect(session.as_deref()).await {
                    Ok(target) => target,
                    Err(error) => {
                        eprintln!("logind listener reconnecting: {error}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };
                let proxy = target.proxy().await?;
                let mut requests = proxy.receive_signal("Lock").await?;
                let mut owners = proxy.receive_owner_changed().await?;
                if !notified {
                    notify_ready()?;
                    notified = true;
                }
                if let Some(status) = child.as_mut().and_then(|child| child.try_wait().ok().flatten()) {
                    if status.success() {
                        let _ = target.set_locked(false).await;
                    }
                    locked = None;
                    child = None;
                    ready = None;
                    if pending {
                        pending = false;
                        launch(&request, &target.id, &mut child, &mut ready);
                    }
                }
                if let Some(locked) = locked {
                    let _ = target.set_locked(locked).await;
                }
                loop {
                    tokio::select! {
                        event = requests.next() => {
                            if event.is_none() { break; }
                            if let Some(status) = child.as_mut().and_then(|c| c.try_wait().ok().flatten()) {
                                if status.success() {
                                    let _ = target.set_locked(false).await;
                                }
                                locked = None;
                                child = None;
                                ready = None;
                            }
                            if child.is_none() {
                                pending = false;
                                launch(&request, &target.id, &mut child, &mut ready);
                            } else if locked == Some(true) && matches!(
                                tokio::time::timeout(DEADLINE, proxy.get_property::<bool>("LockedHint")).await,
                                Ok(Ok(false))
                            ) {
                                // Authenticated unlock can precede helper cleanup/exit.
                                // Preserve a new request during that short unlocked gap.
                                pending = true;
                            }
                        },
                        _ = owners.next() => break,
                        status = async {
                            match child.as_mut() {
                                Some(child) => child.wait().await,
                                None => std::future::pending().await,
                            }
                        } => {
                            if status.is_ok_and(|status| status.success()) {
                                let _ = target.set_locked(false).await;
                            }
                            // Never clear the bus hint on unsuccessful exit. Stop
                            // restoring our private copy once its owner is gone:
                            // a direct recovery locker can now report a new state.
                            locked = None;
                            child = None;
                            ready = None;
                            if pending {
                                pending = false;
                                launch(&request, &target.id, &mut child, &mut ready);
                            }
                        },
                        confirmed = async {
                            use tokio::io::AsyncReadExt;
                            match ready.as_mut() {
                                Some(ready) => {
                                    let mut byte = [0];
                                    ready.read_exact(&mut byte).await.map(|_| byte[0] == b'1')
                                },
                                None => std::future::pending().await,
                            }
                        } => {
                            ready = None;
                            if matches!(confirmed, Ok(true)) {
                                locked = Some(true);
                                let _ = target.set_locked(true).await;
                            }
                        },
                    }
                }
            }
        })
}

fn notify_ready() -> Result<()> {
    use std::os::{
        linux::net::SocketAddrExt,
        unix::net::{SocketAddr, UnixDatagram},
    };
    let Some(path) = std::env::var_os("NOTIFY_SOCKET") else {
        return Ok(());
    };
    use std::os::unix::ffi::OsStrExt;
    let bytes = path.as_bytes();
    let address = if let Some(name) = bytes.strip_prefix(b"@") {
        SocketAddr::from_abstract_name(name)?
    } else {
        SocketAddr::from_pathname(path)?
    };
    UnixDatagram::unbound()?.send_to_addr(b"READY=1", &address)?;
    Ok(())
}
