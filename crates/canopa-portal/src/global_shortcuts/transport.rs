//! An owned FD, pinned compositor identity and bounded asynchronous protocol.
//! Dropping the owner closes authority even while UI/RPC futures still exist.
use anyhow::{Context, Result, anyhow, bail, ensure};
use futures_util::{StreamExt, stream::BoxStream};
use std::{collections::BTreeMap, os::fd::OwnedFd, path::PathBuf, time::Duration};
use tokio::{
    io::AsyncWriteExt,
    net::UnixStream,
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};
use zbus::{Connection, names::OwnedUniqueName};

#[cfg(test)]
mod tests;
mod wire;
const SERVICE: &str = "org.canopa.Niri.GlobalShortcuts1";
const PATH: &str = "/org/canopa/Niri/GlobalShortcuts1";
const CALL_TIME: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub generation: u64,
    pub bindings: BTreeMap<u32, String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    Changed(Snapshot),
    Activated {
        generation: u64,
        binding: u32,
        timestamp_ms: u64,
    },
    Deactivated {
        generation: u64,
        binding: u32,
        timestamp_ms: u64,
    },
}
#[derive(Clone, Debug)]
pub struct Commit {
    pub snapshot: Snapshot,
    pub error: Option<String>,
    pub failed: Vec<u32>,
}
struct Replace {
    expected: u64,
    proposed: BTreeMap<u32, String>,
    reply: oneshot::Sender<Result<Commit>>,
}
#[derive(Clone)]
pub struct Control {
    commands: mpsc::Sender<Replace>,
    snapshot: watch::Receiver<Snapshot>,
    closed: watch::Receiver<bool>,
}
impl Control {
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.borrow().clone()
    }
    pub fn closed(&self) -> watch::Receiver<bool> {
        self.closed.clone()
    }
    pub async fn replace(&self, expected: u64, bindings: Vec<(u32, String)>) -> Result<Commit> {
        ensure!(!*self.closed.borrow(), "Shortcut compositor stopped");
        let proposed = wire::bindings(bindings)?;
        let (reply, result) = oneshot::channel();
        self.commands
            .try_send(Replace {
                expected,
                proposed,
                reply,
            })
            .map_err(|_| anyhow!("Shortcut operation queue is full or closed"))?;
        result.await.context("Shortcut compositor stopped")?
    }
}
pub struct Lease {
    control: Control,
    events: Option<mpsc::Receiver<Notice>>,
    stop: watch::Sender<bool>,
    task: Option<JoinHandle<()>>,
}
impl Lease {
    pub fn control(&self) -> Control {
        self.control.clone()
    }
    pub fn events(&mut self) -> Option<mpsc::Receiver<Notice>> {
        self.events.take()
    }
    pub async fn stop(mut self) {
        self.stop.send_replace(true);
        if let Some(mut task) = self.task.take()
            && tokio::time::timeout(Duration::from_secs(2), &mut task)
                .await
                .is_err()
        {
            task.abort();
            let _ = task.await;
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
#[derive(Clone, Debug)]
pub struct Endpoint {
    owner: OwnedUniqueName,
    pid: u32,
    socket: PathBuf,
}
impl Endpoint {
    pub async fn resolve(connection: &Connection, socket: PathBuf) -> Result<Self> {
        tokio::time::timeout(CALL_TIME, async {
            ensure!(
                socket.is_absolute(),
                "Shortcut compositor socket unavailable"
            );
            let bus = zbus::fdo::DBusProxy::new(connection).await?;
            let owner = bus.get_name_owner(SERVICE.try_into()?).await?;
            let pid = bus
                .get_connection_unix_process_id(owner.clone().into())
                .await?;
            let ipc = UnixStream::connect(&socket).await?;
            ensure!(
                ipc.peer_cred()?.pid().and_then(|p| u32::try_from(p).ok()) == Some(pid),
                "Shortcut provider belongs to another compositor"
            );
            let proxy = proxy(connection, owner.clone()).await?;
            ensure!(
                proxy.get_property::<u32>("Version").await? == 1,
                "Unsupported shortcut provider"
            );
            ensure!(
                bus.get_name_owner(SERVICE.try_into()?).await? == owner,
                "Shortcut owner changed during discovery"
            );
            Ok(Self { owner, pid, socket })
        })
        .await?
    }
    pub async fn open(&self, connection: &Connection) -> Result<Lease> {
        tokio::time::timeout(CALL_TIME, async {
            let bus = zbus::fdo::DBusProxy::new(connection).await?;
            let owners = bus
                .receive_name_owner_changed_with_args(&[(0, SERVICE)])
                .await?;
            ensure!(
                bus.get_name_owner(SERVICE.try_into()?).await? == self.owner
                    && bus
                        .get_connection_unix_process_id(self.owner.clone().into())
                        .await?
                        == self.pid,
                "Shortcut owner changed"
            );
            let ipc = UnixStream::connect(&self.socket).await?;
            ensure!(
                ipc.peer_cred()?.pid().and_then(|p| u32::try_from(p).ok()) == Some(self.pid),
                "Shortcut compositor changed process"
            );
            let message = connection
                .call_method(
                    Some(self.owner.as_str()),
                    PATH,
                    Some(SERVICE),
                    "OpenShortcuts",
                    &(),
                )
                .await?;
            ensure!(
                message.header().unix_fds() == Some(1),
                "Invalid shortcut descriptor count"
            );
            let fd: zbus::zvariant::OwnedFd = message.body().deserialize()?;
            let socket = socket(fd.into(), self.pid)?;
            let events = owners
                .map(|signal| {
                    signal
                        .args()
                        .ok()
                        .and_then(|args| args.new_owner().as_ref().map(ToString::to_string))
                })
                .boxed();
            initialize(socket, events, self.owner.to_string()).await
        })
        .await?
    }
}
async fn proxy(connection: &Connection, owner: OwnedUniqueName) -> zbus::Result<zbus::Proxy<'_>> {
    zbus::proxy::Builder::<zbus::Proxy<'_>>::new(connection)
        .destination(owner)?
        .path(PATH)?
        .interface(SERVICE)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
}
fn socket(fd: OwnedFd, pid: u32) -> Result<UnixStream> {
    use rustix::net::{self, AddressFamily, SocketType};
    ensure!(
        net::sockopt::socket_type(&fd)? == SocketType::STREAM
            && net::sockopt::socket_domain(&fd)? == AddressFamily::UNIX
            && u32::try_from(
                net::sockopt::socket_peercred(&fd)?
                    .pid
                    .as_raw_nonzero()
                    .get()
            )
            .ok()
                == Some(pid),
        "Invalid shortcut descriptor origin"
    );
    let socket = std::os::unix::net::UnixStream::from(fd);
    socket.set_nonblocking(true)?;
    Ok(UnixStream::from_std(socket)?)
}
async fn initialize(
    mut socket: UnixStream,
    mut owners: BoxStream<'static, Option<String>>,
    owner: String,
) -> Result<Lease> {
    socket
        .write_all(&wire::frame(1, serde_json::json!({"op":"list"}))?)
        .await?;
    let mut reader = wire::Reader::default();
    let mut initial = Snapshot::default();
    loop {
        tokio::select! {biased;
            changed=owners.next()=>{ensure!(matches!(changed,Some(Some(ref current)) if current==&owner),"Shortcut owner lost during setup");}
            ready=socket.readable()=>{
                ready?;
                let Some(event)=reader.next(&socket)? else {continue;};
                match event {
                    wire::Event::Changed {generation,bindings,failed}=>{
                        ensure!(generation>initial.generation&&bindings.is_empty()&&failed.is_empty(),"Fresh shortcut lease contains authority");initial.generation=generation;
                    }
                    wire::Event::Reply {sequence,generation,error,bindings,failed}=>{
                        ensure!(sequence==1&&generation>=initial.generation&&error.is_none()&&bindings.is_empty()&&failed.is_empty(),"Invalid initial shortcut snapshot");initial.generation=generation;break;
                    }
                    _=>bail!("Unexpected shortcut activation before registration"),
                }
            }
        }
    }
    let (commands, inbox) = mpsc::channel(4);
    let (events, notifications) = mpsc::channel(64);
    let (state, snapshot) = watch::channel(initial.clone());
    let (stop, closed) = watch::channel(false);
    let control = Control {
        commands,
        snapshot,
        closed,
    };
    let driver_stop = stop.clone();
    let task = tokio::spawn(async move {
        let _closed = Closed(driver_stop.clone());
        let _ = drive(
            socket,
            reader,
            owners,
            owner,
            wire::Ledger::initial(initial).expect("empty initial authority"),
            inbox,
            state,
            events,
            driver_stop,
        )
        .await;
    });
    Ok(Lease {
        stop,
        control,
        events: Some(notifications),
        task: Some(task),
    })
}
struct Closed(watch::Sender<bool>);
impl Drop for Closed {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}
struct ReaderTask(JoinHandle<()>);
impl Drop for ReaderTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}
async fn ended(receiver: &mut watch::Receiver<bool>) {
    while !*receiver.borrow() {
        if receiver.changed().await.is_err() {
            return;
        }
    }
}
struct Pending {
    sequence: u64,
    request: Replace,
    deadline: tokio::time::Instant,
}
#[allow(clippy::too_many_arguments)]
async fn drive(
    socket: UnixStream,
    mut reader: wire::Reader,
    mut owners: BoxStream<'static, Option<String>>,
    owner: String,
    mut ledger: wire::Ledger,
    mut commands: mpsc::Receiver<Replace>,
    state: watch::Sender<Snapshot>,
    events: mpsc::Sender<Notice>,
    stop: watch::Sender<bool>,
) -> Result<()> {
    let mut closing = stop.subscribe();
    // Dedicated reading is never canceled by an RPC/owner select branch. Its
    // partial header/payload remains owned until the entire connection ends.
    let (read, mut write) = socket.into_split();
    let (frames, mut incoming) = mpsc::channel(64);
    let mut read_closed = closing.clone();
    let reader_stop = stop.clone();
    let _reader = ReaderTask(tokio::spawn(async move {
        let _closed = Closed(reader_stop);
        loop {
            tokio::select! {biased;
                _=ended(&mut read_closed)=>break,
                ready=read.readable()=>{
                    if ready.is_err(){break;}
                    // Reader uses the same UnixStream behind the owned half.
                    let mut finish=false;
                    for _ in 0..8 {
                        match reader.next(read.as_ref()) {
                            Ok(Some(event))=>{if frames.try_send(Ok(event)).is_err(){finish=true;break;}},
                            Ok(None)=>break,
                            Err(error)=>{let _=frames.try_send(Err(error));finish=true;break;}
                        }
                    }
                    if finish {break;}
                    tokio::task::yield_now().await;
                }
            }
        }
    }));
    let mut sequence = 1u64;
    let mut pending: Option<Pending> = None;
    loop {
        ensure!(
            !pending
                .as_ref()
                .is_some_and(|p| p.request.reply.is_closed()),
            "Shortcut mutation recipient disappeared"
        );
        let deadline = pending.as_ref().map_or_else(
            || tokio::time::Instant::now() + Duration::from_secs(86400),
            |p| p.deadline,
        );
        tokio::select! {biased;
            _=ended(&mut closing)=>break,
            changed=owners.next()=>{ensure!(matches!(changed,Some(Some(ref current)) if current==&owner),"Shortcut owner lost");}
            _=tokio::time::sleep_until(deadline),if pending.is_some()=>bail!("Shortcut operation timed out"),
            frame=incoming.recv()=>{
                let event=frame.ok_or_else(||anyhow!("Shortcut reader stopped"))??;
                if matches!(event,wire::Event::Reply{..}) {
                    let pending=pending.take().ok_or_else(||anyhow!("Unsolicited shortcut reply"))?;
                    let commit=ledger.reply(event,pending.sequence,pending.request.expected,&pending.request.proposed)?;
                    if commit.error.is_none() {
                        state.send_replace(commit.snapshot.clone());
                        events.try_send(Notice::Changed(commit.snapshot.clone())).map_err(|_|anyhow!("Shortcut event consumer stalled"))?;
                    }
                    if pending.request.reply.send(Ok(commit)).is_err(){bail!("Shortcut mutation recipient disappeared");}
                } else {
                    let notice=ledger.event(event)?;state.send_if_modified(|current|{if *current==ledger.snapshot {false}else{*current=ledger.snapshot.clone();true}});
                    events.try_send(notice).map_err(|_|anyhow!("Shortcut event consumer stalled"))?;
                }
            }
            request=commands.recv(),if pending.is_none()=>{
                let Some(request)=request else {break;};
                sequence=sequence.checked_add(1).ok_or_else(||anyhow!("Shortcut sequence exhausted"))?;
                let bindings:Vec<_>=request.proposed.iter().map(|(id,trigger)|(*id,trigger.clone())).collect();
                let bytes=wire::frame(sequence,serde_json::json!({"op":"replace","generation":request.expected,"bindings":bindings}))?;
                tokio::select! {biased;_=ended(&mut closing)=>break,result=tokio::time::timeout(CALL_TIME,write.write_all(&bytes))=>{result??;}}
                pending=Some(Pending {sequence,request,deadline:tokio::time::Instant::now()+CALL_TIME});
            }
        }
    }
    Ok(())
}
