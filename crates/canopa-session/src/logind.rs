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
pub async fn manager(target: &Target) -> Result<Proxy<'_>> {
    Ok(zbus::proxy::Builder::new(&target.connection)
        .destination(SERVICE)?
        .path(MANAGER)?
        .interface("org.freedesktop.login1.Manager")?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?)
}
pub async fn selected(selected: Option<&str>) -> Result<Target> {
    connect(selected).await
}
async fn sleep_inhibitor(manager: &Proxy<'_>) -> Result<std::os::fd::OwnedFd> {
    let fd: zbus::zvariant::OwnedFd = tokio::time::timeout(
        DEADLINE,
        manager.call(
            "Inhibit",
            &(
                "sleep",
                "Canopa Lock",
                "Wait for compositor-confirmed session lock",
                "delay",
            ),
        ),
    )
    .await
    .context("sleep inhibitor timed out")??;
    Ok(fd.into())
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
    let mut command = std::process::Command::new(canopa_file::executable::current()?);
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
struct Locker {
    request: canopa_config::Request,
    session: String,
    child: Option<tokio::process::Child>,
    ready: Option<tokio::net::UnixStream>,
    locked: Option<bool>,
    generation: u64,
    pending: bool,
    sleep_coordinator: bool,
    preparing_for_sleep: bool,
    waiters: crate::control::Waiters,
    /// The compositor keeps the session locked after its locker died; the next
    /// launch restores the authentication prompt.
    recovering: bool,
    recoveries: std::collections::VecDeque<std::time::Instant>,
    recover_at: Option<tokio::time::Instant>,
}
/// A locker that dies after the compositor confirmed the lock leaves the
/// session locked with nothing to unlock it. Restart it after the compositor
/// has seen the old client disconnect, within a budget that stops a crashing
/// locker from looping.
const RECOVERY_DELAY: Duration = Duration::from_millis(750);
const RECOVERY_WINDOW: Duration = Duration::from_secs(60);
const RECOVERY_LIMIT: usize = 5;
enum Listen {
    Reconnect,
    Stopped,
}
impl Locker {
    fn new(request: canopa_config::Request, session: String) -> Self {
        Self {
            request,
            session,
            child: None,
            ready: None,
            locked: None,
            generation: 0,
            pending: false,
            sleep_coordinator: false,
            preparing_for_sleep: false,
            waiters: Default::default(),
            recovering: false,
            recoveries: Default::default(),
            recover_at: None,
        }
    }
    fn launch(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = false;
        self.recover_at = None;
        match spawn_lock(&self.request, &self.session) {
            Ok((child, ready)) => {
                self.child = Some(child);
                self.ready = Some(ready);
                self.locked = None;
            }
            Err(error) => {
                eprintln!("cannot launch locker: {error}");
                self.waiters.finished(self.generation, false);
            }
        }
    }
    async fn finished(&mut self, target: &Target, success: bool) {
        let lost = self.lost(success);
        if success {
            self.recovering = false;
            let _ = target.set_locked(false).await;
        }
        // Unsuccessful locker exit never manufactures an unlocked hint.
        self.waiters.finished(self.generation, success);
        self.child = None;
        self.ready = None;
        self.locked = None;
        if lost {
            self.schedule_recovery(std::time::Instant::now());
        }
        if self.pending {
            self.launch();
        }
    }
    /// Whether this exit leaves the compositor locked without a locker. The
    /// locker exits successfully only after an authenticated unlock or when the
    /// compositor itself ended the lock.
    fn lost(&self, success: bool) -> bool {
        !success && (self.locked == Some(true) || self.recovering)
    }
    fn schedule_recovery(&mut self, now: std::time::Instant) {
        while self
            .recoveries
            .front()
            .is_some_and(|started| now.duration_since(*started) > RECOVERY_WINDOW)
        {
            self.recoveries.pop_front();
        }
        if self.recoveries.len() >= RECOVERY_LIMIT {
            // The compositor still holds the lock. A later Lock request, such as
            // `loginctl lock-session` from a text console, starts a new locker.
            self.recovering = false;
            self.recover_at = None;
            eprintln!(
                "locker keeps exiting while the session is locked; not restarting it again. \
                 Run `loginctl lock-session` from a text console to retry"
            );
            return;
        }
        self.recoveries.push_back(now);
        self.recovering = true;
        self.recover_at = Some(tokio::time::Instant::now() + RECOVERY_DELAY);
        eprintln!("locker exited while the session was locked; restarting it");
    }
    fn recover(&mut self) {
        self.recover_at = None;
        if self.child.is_none() {
            self.launch();
        }
    }
    async fn reap(&mut self, target: &Target) {
        if let Some(status) = self
            .child
            .as_mut()
            .and_then(|c| c.try_wait().ok().flatten())
        {
            self.finished(target, status.success()).await;
        }
    }
    async fn request(&mut self, target: &Target, request: Option<crate::control::Request>) {
        use crate::control::{Mode, Outcome, Status};
        self.reap(target).await;
        if request
            .as_ref()
            .is_some_and(|r| matches!(r.mode, Mode::Status | Mode::Watch))
        {
            let _ = request.unwrap().reply.send(Outcome::Status(Status {
                version: 1,
                session: self.session.clone(),
                ready: self.child.is_some() && self.locked == Some(true),
                pid: self.child.as_ref().and_then(|c| c.id()),
                generation: self.generation,
                sleep_coordinator: self.sleep_coordinator,
                preparing_for_sleep: self.preparing_for_sleep,
            }));
            return;
        }
        // Authenticated unlock can precede process cleanup. Requests arriving in
        // that gap belong to the next generation and must not complete with it.
        let unlocking = self.child.is_some()
            && self.locked == Some(true)
            && matches!(
                tokio::time::timeout(DEADLINE, async {
                    target
                        .proxy()
                        .await?
                        .get_property::<bool>("LockedHint")
                        .await
                        .map_err(anyhow::Error::from)
                })
                .await,
                Ok(Ok(false))
            );
        let next = self.child.is_none() || unlocking || self.pending;
        if let Some(request) = request {
            self.waiters.add(
                request,
                if next {
                    self.generation.wrapping_add(1)
                } else {
                    self.generation
                },
            );
        }
        if self.child.is_none() {
            self.launch();
        } else if unlocking {
            self.pending = true;
        } else if self.locked == Some(true) && !self.pending {
            self.waiters.ready(self.generation);
        }
    }
}
pub fn daemon(request: canopa_config::Request, selected: Option<String>) -> Result<()> {
    // Report a configuration mistake without refusing to listen: each locker
    // falls back to defaults rather than leaving the session unlockable.
    if let Err(error) =
        crate::settings::Settings::load(&request, canopa_config::Component::Lockscreen)
    {
        eprintln!(
            "lockscreen configuration is invalid; locking with defaults until it is fixed: {error:#}"
        );
    }
    std::fs::metadata("/etc/pam.d/canopa-lock")
        .context("install canopa-lock PAM service before starting its listener")?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async move {
            let session = connect(selected.as_deref()).await?.id;
            let _guard = daemon_guard(&session)?;
            let server = crate::control::Server::bind()?;
            let (sender, mut controls) = mpsc::channel(32);
            let _server = tokio::spawn(server.serve(sender));
            let mut locker = Locker::new(request, session.clone());
            let mut notified = false;
            loop {
                // Losing logind or one of its subscriptions must not end the
                // listener: Lock and PrepareForSleep signals sent while systemd
                // restarted it would be lost. The managed locker keeps running.
                match locker.listen(&session, &mut controls, &mut notified).await {
                    Ok(Listen::Reconnect) => {}
                    Ok(Listen::Stopped) => bail!("locker control stopped"),
                    Err(error) => {
                        eprintln!("logind listener reconnecting: {error:#}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        })
}
impl Locker {
    async fn listen(
        &mut self,
        session: &str,
        controls: &mut mpsc::Receiver<crate::control::Request>,
        notified: &mut bool,
    ) -> Result<Listen> {
        let target = connect(Some(session)).await?;
        let proxy = target.proxy().await?;
        let mut requests = proxy.receive_signal("Lock").await?;
        let mut owners = proxy.receive_owner_changed().await?;
        let manager = manager(&target).await?;
        let mut sleep = manager.receive_signal("PrepareForSleep").await?;
        let mut inhibitor = Some(sleep_inhibitor(&manager).await?);
        self.preparing_for_sleep = manager.get_property::<bool>("PreparingForSleep").await?;
        self.sleep_coordinator = true;
        if self.preparing_for_sleep {
            self.request(&target, None).await;
            if self.locked == Some(true) && !self.pending {
                inhibitor.take();
            }
        }
        if !*notified {
            notify_ready()?;
            *notified = true;
        }
        self.reap(&target).await;
        if let Some(locked) = self.locked {
            let _ = target.set_locked(locked).await;
        }
        loop {
            tokio::select! {
                event = sleep.next() => {
                    let Some(event) = event else { return Ok(Listen::Reconnect) };
                    let preparing: bool = event.body().deserialize()?;
                    self.preparing_for_sleep = preparing;
                    if preparing {
                        self.request(&target, None).await;
                        if self.locked == Some(true) && !self.pending {
                            inhibitor.take();
                        }
                    } else {
                        inhibitor.take();
                        inhibitor = Some(sleep_inhibitor(&manager).await?);
                    }
                },
                event = requests.next() => {
                    if event.is_none() {
                        return Ok(Listen::Reconnect);
                    }
                    self.request(&target, None).await;
                },
                request = controls.recv() => {
                    let Some(request) = request else { return Ok(Listen::Stopped) };
                    self.request(&target, Some(request)).await;
                },
                _ = owners.next() => return Ok(Listen::Reconnect),
                status = async {
                    match self.child.as_mut() {
                        Some(child) => child.wait().await,
                        None => std::future::pending().await,
                    }
                } => {
                    self.finished(&target, status.is_ok_and(|s| s.success())).await;
                },
                _ = async {
                    match self.recover_at {
                        Some(at) => tokio::time::sleep_until(at).await,
                        None => std::future::pending().await,
                    }
                } => self.recover(),
                confirmed = async {
                    use tokio::io::AsyncReadExt;
                    match self.ready.as_mut() {
                        Some(ready) => ready.read_u8().await,
                        None => std::future::pending().await,
                    }
                } => {
                    self.ready = None;
                    if crate::control::failed_readiness(confirmed).is_ok() {
                        self.locked = Some(true);
                        self.recovering = false;
                        let _ = target.set_locked(true).await;
                        self.waiters.ready(self.generation);
                        if self.preparing_for_sleep && !self.pending {
                            inhibitor.take();
                        }
                    } else {
                        self.waiters.finished(self.generation, false);
                    }
                },
            }
        }
    }
}

pub(crate) fn notify_ready() -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn locker() -> Locker {
        Locker::new(canopa_config::Request::Defaults, "1".into())
    }

    #[test]
    fn only_a_failed_exit_after_confirmed_lock_is_recovered() {
        let mut locker = locker();
        // Refused or failed before the compositor confirmed the lock: the
        // session never locked, so restarting would only repeat the failure.
        assert!(!locker.lost(false));
        locker.locked = Some(true);
        assert!(!locker.lost(true), "authenticated unlock");
        assert!(locker.lost(false), "crash while locked");
        // A replacement that dies before confirming is still guarding a lock.
        locker.locked = None;
        locker.recovering = true;
        assert!(locker.lost(false));
    }

    #[test]
    fn recovery_is_delayed_and_bounded() {
        let mut locker = locker();
        let start = std::time::Instant::now();
        for attempt in 0..RECOVERY_LIMIT {
            locker.schedule_recovery(start + Duration::from_secs(attempt as u64));
            assert!(locker.recovering);
            assert!(locker.recover_at.is_some());
            locker.recover_at = None;
        }
        locker.schedule_recovery(start + Duration::from_secs(10));
        assert!(!locker.recovering, "a crashing locker must not loop");
        assert!(locker.recover_at.is_none());
        // Old attempts leave the window, so a later crash is recovered again.
        locker.schedule_recovery(
            start + RECOVERY_WINDOW + Duration::from_secs(RECOVERY_LIMIT as u64),
        );
        assert!(locker.recovering);
        assert!(locker.recover_at.is_some());
    }
}
