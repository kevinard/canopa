//! A bounded parent actor owns control; only the EI socket is handed to its app.
use super::{
    wire::{self, Frame, ParentCommand, ParentRequest, ReadMessage, Reader},
    *,
};
use crate::process::{DialogProcess, fd};
use std::{io::Write, os::fd::OwnedFd, process::Stdio, sync::Arc};
use tokio::{
    io::unix::AsyncFd,
    sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot},
};

struct Operation {
    command: ParentCommand,
    reply: oneshot::Sender<anyhow::Result<Reply>>,
}
pub struct Bridge {
    client: Option<OwnedFd>,
    operations: mpsc::Sender<Operation>,
    stop: watch::Sender<bool>,
    closed: watch::Receiver<bool>,
    snapshot: watch::Receiver<Snapshot>,
    events: Option<mpsc::Receiver<Notice>>,
    done: Option<oneshot::Receiver<anyhow::Result<()>>>,
    pid: u32,
}
#[derive(Clone)]
pub struct Revoker(watch::Sender<bool>);
impl Revoker {
    pub fn revoke(&self) {
        self.0.send_replace(true);
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop.send_replace(true);
    }
}
impl Bridge {
    pub fn worker_pid(&self) -> u32 {
        self.pid
    }
    pub fn revoker(&self) -> Revoker {
        Revoker(self.stop.clone())
    }
    pub fn closed(&self) -> watch::Receiver<bool> {
        self.closed.clone()
    }
    pub fn snapshot(&self) -> watch::Receiver<Snapshot> {
        self.snapshot.clone()
    }
    pub fn take_events(&mut self) -> Option<mpsc::Receiver<Notice>> {
        self.events.take()
    }
    async fn perform(&self, command: ParentCommand) -> anyhow::Result<Reply> {
        anyhow::ensure!(
            !*self.closed.borrow() && !*self.stop.borrow(),
            "Capture lease closed"
        );
        let (reply, receive) = oneshot::channel();
        if self
            .operations
            .try_send(Operation { command, reply })
            .is_err()
        {
            self.stop.send_replace(true);
            anyhow::bail!("Capture command queue unavailable");
        }
        receive.await?
    }
    pub async fn command(&self, command: Command) -> anyhow::Result<Reply> {
        command.validate()?;
        self.perform(ParentCommand::Capture { command }).await
    }
    pub async fn connect(&mut self) -> anyhow::Result<OwnedFd> {
        let client = self
            .client
            .take()
            .ok_or_else(|| anyhow::anyhow!("EI connection already handed off"))?;
        anyhow::ensure!(
            matches!(self.perform(ParentCommand::Connect).await?, Reply::Unit),
            "Invalid EI handoff reply"
        );
        Ok(client)
    }
    pub async fn stop(mut self) -> anyhow::Result<()> {
        self.stop.send_replace(true);
        self.client.take();
        self.done.take().expect("capture completion").await?
    }
}

#[derive(Clone)]
pub struct Connector {
    process: Arc<DialogProcess>,
    slots: Arc<Semaphore>,
}
impl Connector {
    pub fn new(process: Arc<DialogProcess>) -> Self {
        Self {
            process,
            slots: Arc::new(Semaphore::new(8)),
        }
    }
    /// Call after fresh approval. Discovery alone never opens this resource.
    pub async fn start(
        &self,
        connection: &Connection,
        endpoint: &Endpoint,
        caller: OwnedUniqueName,
        grants: u32,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Bridge> {
        anyhow::ensure!(!*cancelled.borrow(), "Capture start cancelled");
        anyhow::ensure!(
            grants != 0 && grants & !endpoint.capabilities() == 0,
            "Capture grant is unavailable"
        );
        let input = Input {
            compositor: endpoint.owner.to_string(),
            compositor_pid: endpoint.pid,
            ipc_socket: endpoint.socket.clone(),
            backend: connection
                .unique_name()
                .ok_or_else(|| anyhow::anyhow!("Capture backend needs a bus connection"))?
                .to_string(),
            caller: caller.to_string(),
            grants,
        };
        input.validate()?;
        let permit = self.slots.clone().try_acquire_owned()?;
        launch(&self.process, input, cancelled, permit).await
    }
}
async fn stopping(stop: &mut watch::Receiver<bool>) {
    while !*stop.borrow() {
        if stop.changed().await.is_err() {
            return;
        }
    }
}
async fn send_start(output: &AsyncFd<UnixStream>, input: &Input) -> anyhow::Result<()> {
    let bytes = wire::encode(input)?;
    let mut offset = 0;
    while offset < bytes.len() {
        let mut ready = output.writable().await?;
        match ready.try_io(|output| output.get_ref().write(&bytes[offset..])) {
            Ok(Ok(0)) => anyhow::bail!("Capture bootstrap closed"),
            Ok(Ok(n)) => offset += n,
            Ok(Err(error)) => return Err(error.into()),
            Err(_) => {}
        }
    }
    Ok(())
}
async fn launch(
    process: &DialogProcess,
    input: Input,
    mut cancelled: watch::Receiver<bool>,
    permit: OwnedSemaphorePermit,
) -> anyhow::Result<Bridge> {
    let (parent, child_socket) = UnixStream::pair()?;
    parent.set_nonblocking(true)?;
    // SAFETY: the stream owns its socket descriptor until the AsyncFd is dropped.
    let output = unsafe { AsyncFd::register(parent) }.map_err(std::io::Error::from)?;
    let mut child = tokio::process::Command::new(&process.executable)
        .args(&process.arguments)
        .arg("--input-capture-worker")
        .stdin(Stdio::from(OwnedFd::from(child_socket)))
        .stdout(Stdio::null())
        .env_remove("XDG_ACTIVATION_TOKEN")
        .env_remove("DESKTOP_STARTUP_ID")
        .kill_on_drop(false)
        .spawn()?;
    let pid = child.id().expect("capture worker PID");
    let (stop, mut stopped) = watch::channel(false);
    let (closed, closed_state) = watch::channel(false);
    let (snapshot, snapshot_state) = watch::channel(Snapshot::new(0, Vec::new()));
    let (events, event_state) = mpsc::channel(32);
    let (commands, operations) = mpsc::channel(16);
    let (start, started) = oneshot::channel();
    let (done, finished) = oneshot::channel();
    tokio::spawn(async move {
        let startup = async {
            send_start(&output, &input).await?;
            let client = loop {
                let mut ready = output.readable().await?;
                match ready.try_io(|output| fd::receive(output.get_ref(), pid)) {
                    Ok(value) => break value?,
                    Err(_) => continue,
                }
            };
            let mut stream = tokio::net::UnixStream::from_std(output.into_inner())?;
            let Frame::Ready { zone_set, zones } = wire::read(&mut stream).await? else {
                anyhow::bail!("Missing capture readiness");
            };
            wire::validate_zones(&zones)?;
            snapshot.send_replace(Snapshot::new(zone_set, zones));
            Ok::<_, anyhow::Error>((client, stream))
        };
        let prepared = tokio::select! {
            biased;
            _=stopping(&mut stopped)=>Err(anyhow::anyhow!("Capture start cancelled")),
            _=stopping(&mut cancelled)=>Err(anyhow::anyhow!("Capture start cancelled")),
            result=tokio::time::timeout(Duration::from_secs(30),startup)=>result.map_err(anyhow::Error::from).and_then(|v|v),
        };
        drop(cancelled);
        match prepared {
            Ok((client, stream)) => {
                if start.send(Ok(client)).is_ok()
                    && let Err(error) =
                        serve(stream, operations, &snapshot, &events, &mut stopped).await
                {
                    eprintln!(
                        "level=warn component=input-capture event=control-ended error={error}"
                    );
                }
            }
            Err(error) => {
                let _ = start.send(Err(error));
            }
        }
        // Loss of control immediately ends phase-dependent authority, while
        // the admission permit remains owned until actual child retirement.
        snapshot.send_modify(|state| {
            state.enabled = false;
            state.activation = None;
            state.receiver_ready = false;
        });
        let result = match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
            Ok(Ok(status)) if status.success() => Ok(()),
            Ok(_) => Err(anyhow::anyhow!("Capture worker cleanup unconfirmed")),
            Err(_) => {
                let _ = child.kill().await;
                Err(anyhow::anyhow!("Capture worker did not retire"))
            }
        };
        drop(permit);
        snapshot.send_modify(|state| {
            state.enabled = false;
            state.activation = None;
            state.receiver_ready = false;
        });
        closed.send_replace(true);
        let _ = done.send(result);
    });
    let client = match started.await {
        Ok(Ok(client)) => client,
        result => {
            let error = match result {
                Ok(Err(error)) => error,
                Err(error) => error.into(),
                _ => unreachable!(),
            };
            let _ = finished.await;
            return Err(error);
        }
    };
    Ok(Bridge {
        client: Some(client),
        operations: commands,
        stop,
        closed: closed_state,
        snapshot: snapshot_state,
        events: Some(event_state),
        done: Some(finished),
        pid,
    })
}

struct Pending {
    sequence: u64,
    operation: Operation,
    deadline: tokio::time::Instant,
}
fn accepted(snapshot: &mut Snapshot, command: &ParentCommand, reply: &Reply) -> anyhow::Result<()> {
    match (command, reply) {
        (_, Reply::Failed { .. }) => {}
        (ParentCommand::Connect, Reply::Unit) => snapshot.connected = true,
        (
            ParentCommand::Capture {
                command: Command::GetZones,
            },
            Reply::Zones { zone_set, zones },
        ) => {
            wire::validate_zones(zones)?;
            snapshot.zone_set = *zone_set;
            snapshot.zones = zones.clone();
            snapshot.zones_valid = true;
        }
        (
            ParentCommand::Capture {
                command: Command::SetBarriers { .. },
            },
            Reply::Barriers { .. },
        ) => {
            snapshot.enabled = false;
            snapshot.activation = None;
        }
        (
            ParentCommand::Capture {
                command: Command::Enable,
            },
            Reply::Unit,
        ) => snapshot.enabled = true,
        (
            ParentCommand::Capture {
                command: Command::Disable,
            },
            Reply::Unit,
        ) => {
            snapshot.enabled = false;
            snapshot.activation = None;
        }
        (
            ParentCommand::Capture {
                command: Command::Release { .. },
            },
            Reply::Unit,
        ) => snapshot.activation = None,
        _ => anyhow::bail!("Invalid capture command reply"),
    }
    Ok(())
}
async fn serve(
    stream: tokio::net::UnixStream,
    mut operations: mpsc::Receiver<Operation>,
    snapshot: &watch::Sender<Snapshot>,
    events: &mpsc::Sender<Notice>,
    stop: &mut watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut reader = Reader::<Frame>::new(read, 32);
    let mut pending: Option<Pending> = None;
    let mut sequence = 0u64;
    let result=async {
        loop {
            tokio::select! {
                biased;
                _=stopping(stop)=>return Ok(()),
                _=async {tokio::time::sleep_until(pending.as_ref().unwrap().deadline).await},if pending.is_some()=>anyhow::bail!("Capture operation timed out"),
                message=reader.recv()=>match message {
                    Some(ReadMessage::Data {value:Frame::Notice {notice},..})=>{
                        snapshot.send_modify(|value|value.notice(&notice));
                        events.try_send(notice).map_err(|_|anyhow::anyhow!("Capture event consumer unavailable"))?;
                    }
                    Some(ReadMessage::Data {value:Frame::Reply {sequence,reply},..})=>{
                        let active=pending.take().ok_or_else(||anyhow::anyhow!("Unexpected capture reply"))?;
                        anyhow::ensure!(active.sequence==sequence,"Capture reply sequence mismatch");
                        let mut next=snapshot.borrow().clone();accepted(&mut next,&active.operation.command,&reply)?;
                        snapshot.send_replace(next);
                        let reply=match reply {Reply::Failed {message}=>Err(anyhow::anyhow!(message)),other=>Ok(other)};
                        let _=active.operation.reply.send(reply);
                    }
                    Some(ReadMessage::Error(error))=>anyhow::bail!(error),
                    _=>anyhow::bail!("Capture worker control closed"),
                },
                operation=operations.recv(),if pending.is_none()=>{
                    let Some(operation)=operation else {return Ok(());};
                    sequence=sequence.checked_add(1).ok_or_else(||anyhow::anyhow!("Capture command serial exhausted"))?;
                    let request=ParentRequest {sequence,command:operation.command};
                    tokio::select! {
                        biased;_=stopping(stop)=>return Ok(()),
                        result=tokio::time::timeout(Duration::from_secs(2),wire::write(&mut write,&request))=>result??,
                    }
                    pending=Some(Pending {sequence,operation:Operation {command:request.command,reply:operation.reply},
                        deadline:tokio::time::Instant::now()+Duration::from_secs(10)});
                }
            }
        }
    }.await;
    if let Some(pending) = pending {
        let _ = pending
            .operation
            .reply
            .send(Err(anyhow::anyhow!("Capture lease ended")));
    }
    while let Ok(operation) = operations.try_recv() {
        let _ = operation
            .reply
            .send(Err(anyhow::anyhow!("Capture lease ended")));
    }
    drop(reader);
    drop(write);
    result
}
