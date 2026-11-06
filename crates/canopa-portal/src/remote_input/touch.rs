//! Private touch worker ownership. Public EI/Notify permission stays in the host.
use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) mod compositor;
mod wire;
pub use wire::{Command, Status};

pub struct TouchInput {
    socket: tokio::net::UnixStream,
    lease: InputLease,
    sequence: u32,
    failed: bool,
}
impl TouchInput {
    pub(super) fn new(fd: OwnedFd, lease: InputLease) -> anyhow::Result<Self> {
        let socket = UnixStream::from(fd);
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket: tokio::net::UnixStream::from_std(socket)?,
            lease,
            sequence: 0,
            failed: false,
        })
    }
    pub fn closed(&self) -> watch::Receiver<bool> {
        if self.failed {
            watch::channel(true).1
        } else {
            self.lease.closed()
        }
    }
    pub async fn send(&mut self, command: Command) -> anyhow::Result<Status> {
        if !command.valid() {
            return Ok(Status::Invalid);
        }
        anyhow::ensure!(!*self.closed().borrow(), "Touch worker ended");
        let Some(next) = self.sequence.checked_add(1) else {
            self.failed = true;
            self.lease.revoke();
            anyhow::bail!("Touch sequence exhausted");
        };
        self.failed = true;
        let mut flight = InFlight {
            lease: &mut self.lease,
            completed: false,
        };
        let result = tokio::time::timeout(CALL_TIME, async {
            self.socket.write_all(&command.packet(next, None)).await?;
            let mut response = [0; 16];
            self.socket.read_exact(&mut response).await?;
            wire::status(response, next)
        })
        .await
        .map_err(anyhow::Error::from)
        .and_then(|r| r);
        match result {
            Ok(status) => {
                self.sequence = next;
                self.failed = false;
                flight.completed = true;
                Ok(status)
            }
            Err(error) => Err(error),
        }
    }
    pub async fn stop(self) -> anyhow::Result<()> {
        let Self { socket, lease, .. } = self;
        drop(socket);
        lease.stop().await
    }
}

struct InFlight<'a> {
    lease: &'a mut InputLease,
    completed: bool,
}
impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.lease.revoke();
        }
    }
}

pub(super) async fn run(
    connection: &Connection,
    input: &Input,
    stop: &mut watch::Receiver<bool>,
    ready: impl FnOnce(UnixStream) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut touch = compositor::Touch::open(connection, input).await?;
    let result = async {
        anyhow::ensure!(!*stop.borrow(), "Touch cancelled");
        let (server, client)=UnixStream::pair()?;
        server.set_nonblocking(true)?;
        let mut server=tokio::net::UnixStream::from_std(server)?;
        ready(client)?;
        let mut data=[0;40]; let mut filled=0; let mut sequence=0u32;
        loop {
            if *stop.borrow() { return Ok(()); }
            tokio::select! {
                biased;
                _=stop.changed()=>return Ok(()),
                ended=touch.ended()=>{
                    ended?;
                    if *stop.borrow() || !touch.renew(connection,input).await? { return Ok(()); }
                }
                read=server.read(&mut data[filled..])=>{
                    let n=read?; if n==0 { return Ok(()); } filled+=n;
                    if filled<data.len() { continue; }
                    filled=0; sequence=sequence.checked_add(1).ok_or_else(||anyhow::anyhow!("Touch sequence exhausted"))?;
                    let command=Command::decode(&data,sequence)?;
                    if *stop.borrow() { return Ok(()); }
                    let status=match touch.send(command).await? {
                        Some(status) => status,
                        None => {
                            if *stop.borrow() || !touch.renew(connection,input).await? { return Ok(()); }
                            // The old command may have reached the compositor. A new
                            // source must never replay it; the caller sees unavailability.
                            Status::Unavailable
                        }
                    };
                    tokio::time::timeout(Duration::from_secs(2),server.write_all(&wire::feedback(sequence,status))).await??;
                }
            }
        }
    }.await;
    touch.close().await?;
    result
}
