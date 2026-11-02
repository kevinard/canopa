//! Private, session-scoped locker control. This channel never carries credentials.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{io::ErrorKind, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
    sync::{mpsc, oneshot},
    task::JoinSet,
};

const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Ready,
    Foreground,
    Status,
    Watch,
}
impl Mode {
    fn byte(self) -> u8 {
        match self {
            Self::Ready => b'R',
            Self::Foreground => b'W',
            Self::Status => b'S',
            Self::Watch => b'H',
        }
    }
    fn parse(byte: u8) -> Option<Self> {
        match byte {
            b'R' => Some(Self::Ready),
            b'W' => Some(Self::Foreground),
            b'S' => Some(Self::Status),
            b'H' => Some(Self::Watch),
            _ => None,
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub version: u32,
    pub session: String,
    pub ready: bool,
    pub pid: Option<u32>,
    pub generation: u64,
    pub sleep_coordinator: bool,
    pub preparing_for_sleep: bool,
}
pub enum Outcome {
    Ready,
    Unlocked,
    Status(Status),
    Failed,
}
pub struct Request {
    pub mode: Mode,
    pub reply: oneshot::Sender<Outcome>,
}

pub struct Server {
    listener: UnixListener,
    _files: crate::ipc::Server,
}
impl Server {
    pub fn bind() -> Result<Self> {
        let files = crate::ipc::Server::bind_path(crate::ipc::scoped_socket_path("lock-control")?)?;
        files.listener.set_nonblocking(true)?;
        let listener = UnixListener::from_std(files.listener.try_clone()?)?;
        Ok(Self {
            listener,
            _files: files,
        })
    }
    pub async fn serve(self, sender: mpsc::Sender<Request>) {
        let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(32));
        let mut clients = JoinSet::new();
        loop {
            tokio::select! {
                _=sender.closed()=>break,
                _=clients.join_next(), if !clients.is_empty()=>{},
                accepted=self.listener.accept()=> {
                    let Ok((stream,_))=accepted else {break};
                    let Ok(permit)=permits.clone().try_acquire_owned() else {continue};
                    let sender=sender.clone();
                    clients.spawn(async move {let _permit=permit;let _=serve_client(stream,sender).await;});
                }
            }
        }
        clients.abort_all();
    }
}
fn owned_peer(stream: &UnixStream) -> Result<()> {
    ensure!(
        stream.peer_cred()?.uid() == unsafe { libc::getuid() },
        "locker control belongs to another user"
    );
    Ok(())
}
async fn serve_client(mut stream: UnixStream, sender: mpsc::Sender<Request>) -> Result<()> {
    owned_peer(&stream)?;
    let byte = tokio::time::timeout(Duration::from_secs(1), stream.read_u8()).await??;
    let mode = Mode::parse(byte).context("invalid locker control operation")?;
    let (reply, receiver) = oneshot::channel();
    sender.send(Request { mode, reply }).await?;
    let mut unexpected = [0u8; 1];
    let outcome = tokio::select! {
        result=receiver=>result.unwrap_or(Outcome::Failed),
        _=stream.read(&mut unexpected)=>return Ok(()),
    };
    match outcome {
        Outcome::Ready => stream.write_all(b"R").await?,
        Outcome::Unlocked => stream.write_all(b"U").await?,
        Outcome::Failed => stream.write_all(b"E").await?,
        Outcome::Status(status) => {
            stream.write_all(b"S").await?;
            let bytes = serde_json::to_vec(&status)?;
            ensure!(bytes.len() <= 1024, "locker status exceeds budget");
            stream.write_all(&bytes).await?;
            stream.write_all(b"\n").await?;
        }
    }
    if mode == Mode::Watch {
        tokio::select! {_=sender.closed()=>{},_=stream.read(&mut unexpected)=>{}}
    }
    stream.shutdown().await?;
    Ok(())
}
async fn connect() -> Result<Option<UnixStream>> {
    let path = crate::ipc::scoped_socket_path("lock-control")?;
    match tokio::time::timeout(CONNECT_TIMEOUT, UnixStream::connect(path)).await? {
        Ok(stream) => {
            owned_peer(&stream)?;
            Ok(Some(stream))
        }
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::NotFound | ErrorKind::ConnectionRefused
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error.into()),
    }
}
pub async fn foreground() -> Result<bool> {
    let Some(mut stream) = connect().await? else {
        return Ok(false);
    };
    stream.write_all(&[Mode::Foreground.byte()]).await?;
    ensure!(
        stream.read_u8().await? == b'U',
        "managed locker stopped without an authenticated unlock"
    );
    Ok(true)
}
pub async fn ready(timeout: Duration) -> Result<()> {
    tokio::time::timeout(timeout, async {
        let mut stream = connect()
            .await?
            .context("start the Canopa lock listener before requesting managed lock readiness")?;
        stream.write_all(&[Mode::Ready.byte()]).await?;
        ensure!(
            stream.read_u8().await? == b'R',
            "locker did not confirm compositor lock readiness"
        );
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("locker readiness timed out")?
}
pub async fn status() -> Result<Status> {
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut stream = connect()
            .await?
            .context("Canopa lock listener is unavailable")?;
        stream.write_all(&[Mode::Status.byte()]).await?;
        ensure!(
            stream.read_u8().await? == b'S',
            "invalid locker status response"
        );
        let mut bytes = Vec::new();
        stream.take(1025).read_to_end(&mut bytes).await?;
        ensure!(bytes.len() <= 1024, "locker status exceeds budget");
        let status: Status = serde_json::from_slice(&bytes)?;
        ensure!(status.version == 1, "unsupported locker control version");
        Ok::<_, anyhow::Error>(status)
    })
    .await
    .context("locker status timed out")?
}
/// A lifetime subscription: after the initial status, EOF means the coordinator
/// is gone. No heartbeat or periodic status polling is needed.
pub async fn watch() -> Result<(Status, UnixStream)> {
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut stream = connect()
            .await?
            .context("Canopa lock listener is unavailable")?;
        stream.write_all(&[Mode::Watch.byte()]).await?;
        ensure!(
            stream.read_u8().await? == b'S',
            "invalid locker watch response"
        );
        let mut bytes = Vec::new();
        loop {
            let byte = stream.read_u8().await?;
            if byte == b'\n' {
                break;
            }
            ensure!(bytes.len() < 1024, "locker status exceeds budget");
            bytes.push(byte);
        }
        let status: Status = serde_json::from_slice(&bytes)?;
        ensure!(status.version == 1, "unsupported locker control version");
        Ok::<_, anyhow::Error>((status, stream))
    })
    .await
    .context("locker watch timed out")?
}

struct Waiter {
    mode: Mode,
    generation: u64,
    reply: oneshot::Sender<Outcome>,
}
#[derive(Default)]
pub(crate) struct Waiters(Vec<Waiter>);
impl Waiters {
    pub fn add(&mut self, request: Request, generation: u64) {
        self.0.retain(|w| !w.reply.is_closed());
        if self.0.len() >= 32 {
            let _ = request.reply.send(Outcome::Failed);
            return;
        }
        self.0.push(Waiter {
            mode: request.mode,
            generation,
            reply: request.reply,
        });
    }
    pub fn ready(&mut self, generation: u64) {
        let mut rest = Vec::new();
        for waiter in self.0.drain(..) {
            if waiter.generation == generation && waiter.mode == Mode::Ready {
                let _ = waiter.reply.send(Outcome::Ready);
            } else if !waiter.reply.is_closed() {
                rest.push(waiter);
            }
        }
        self.0 = rest;
    }
    pub fn finished(&mut self, generation: u64, success: bool) {
        let mut rest = Vec::new();
        for waiter in self.0.drain(..) {
            if waiter.generation == generation {
                let result = if success && waiter.mode == Mode::Foreground {
                    Outcome::Unlocked
                } else {
                    Outcome::Failed
                };
                let _ = waiter.reply.send(result);
            } else if !waiter.reply.is_closed() {
                rest.push(waiter);
            }
        }
        self.0 = rest;
    }
}
pub(crate) fn failed_readiness(result: std::io::Result<u8>) -> Result<()> {
    match result {
        Ok(b'1') => Ok(()),
        _ => bail!("locker exited before confirming readiness"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(mode: Mode) -> (Request, oneshot::Receiver<Outcome>) {
        let (reply, receiver) = oneshot::channel();
        (Request { mode, reply }, receiver)
    }
    #[test]
    fn readiness_does_not_finish_foreground_or_future_generations() {
        let mut waiters = Waiters::default();
        let (a, mut ready) = request(Mode::Ready);
        let (b, mut foreground) = request(Mode::Foreground);
        let (c, mut future) = request(Mode::Ready);
        waiters.add(a, 1);
        waiters.add(b, 1);
        waiters.add(c, 2);
        waiters.ready(1);
        assert!(matches!(ready.try_recv(), Ok(Outcome::Ready)));
        assert!(foreground.try_recv().is_err());
        assert!(future.try_recv().is_err());
        waiters.finished(1, true);
        assert!(matches!(foreground.try_recv(), Ok(Outcome::Unlocked)));
        assert!(future.try_recv().is_err());
        waiters.finished(2, false);
        assert!(matches!(future.try_recv(), Ok(Outcome::Failed)));
    }
    #[test]
    fn failure_never_reports_ready_or_unlocked_and_cancelled_clients_retire() {
        let mut waiters = Waiters::default();
        let (a, mut ready) = request(Mode::Ready);
        let (b, mut foreground) = request(Mode::Foreground);
        waiters.add(a, 7);
        waiters.add(b, 7);
        waiters.finished(7, false);
        assert!(matches!(ready.try_recv(), Ok(Outcome::Failed)));
        assert!(matches!(foreground.try_recv(), Ok(Outcome::Failed)));
        for _ in 0..40 {
            let (request, receiver) = request(Mode::Ready);
            drop(receiver);
            waiters.add(request, 8);
        }
        assert!(waiters.0.len() <= 1);
        assert!(failed_readiness(Ok(b'0')).is_err());
        assert!(failed_readiness(Err(std::io::ErrorKind::UnexpectedEof.into())).is_err());
    }
}
