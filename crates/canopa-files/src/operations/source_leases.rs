//! Process-owned retention, with final release kept off the caller's UI thread.
use super::{requests::RetainedSources, store::ReadSource, *};
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread::JoinHandle,
};

/// Opaque retention ownership, not authority to execute or replay a transfer.
/// Clones share one reservation; independent acquisitions own separate ones.
#[derive(Clone)]
pub struct RetainedSourceLease {
    history: Identity,
    owner: Arc<Lease>,
}

impl std::fmt::Debug for RetainedSourceLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedSourceLease")
            .finish_non_exhaustive()
    }
}

impl RetainedSourceLease {
    pub(super) fn reservation(&self) -> io::Result<(Identity, u64)> {
        Ok((self.history, self.owner.id.ok_or_else(stopped)?))
    }

    /// Worker-only. Release this owner, waiting for metadata deletion when it is
    /// the last clone. After engine shutdown, the final owner also waits for
    /// history-lock retirement. Other clones keep their protection. Ordinary
    /// Drop queues the same deletion without blocking or touching the filesystem.
    pub fn release(self) -> io::Result<()> {
        if let Some(mut lease) = Arc::into_inner(self.owner) {
            let lifetime = lease.lifetime.clone();
            let (send, receive) = mpsc::sync_channel(1);
            let result = lease.enqueue(Some(send));
            drop(lease);
            result?;
            receive.recv().map_err(|_| stopped())??;
            lifetime.wait_if_closed();
        }
        Ok(())
    }
}

type Reply = mpsc::SyncSender<io::Result<()>>;
enum Command {
    Release(u64, Option<Reply>),
    Flush(Reply),
}

struct Lease {
    id: Option<u64>,
    sender: mpsc::Sender<Command>,
    lifetime: Arc<WorkerLifetime>,
}

impl Lease {
    fn enqueue(&mut self, reply: Option<Reply>) -> io::Result<()> {
        if let Some(id) = self.id.take() {
            let result = self
                .sender
                .send(Command::Release(id, reply))
                .map_err(|_| stopped());
            self.lifetime.owners.fetch_sub(1, Ordering::Release);
            result
        } else {
            Ok(())
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let _ = self.enqueue(None);
    }
}

pub(super) struct Releaser {
    sender: Option<mpsc::Sender<Command>>,
    lifetime: Arc<WorkerLifetime>,
    worker: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct WorkerLifetime {
    owners: AtomicUsize,
    closed: AtomicBool,
    finished: Mutex<bool>,
    wake: Condvar,
}

impl WorkerLifetime {
    fn wait_if_closed(&self) {
        if self.closed.load(Ordering::Acquire) && self.owners.load(Ordering::Acquire) == 0 {
            let mut finished = self
                .finished
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            while !*finished {
                finished = self
                    .wake
                    .wait(finished)
                    .unwrap_or_else(|error| error.into_inner());
            }
        }
    }
}

struct WorkerExit(Arc<WorkerLifetime>);

impl Drop for WorkerExit {
    fn drop(&mut self) {
        *self
            .0
            .finished
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = true;
        self.0.wake.notify_all();
    }
}

impl Releaser {
    fn start(reads: ReadSource, lock: Arc<history_lock::HistoryLock>) -> io::Result<Self> {
        let (send, receive) = mpsc::channel();
        let lifetime = Arc::new(WorkerLifetime::default());
        let worker_lifetime = lifetime.clone();
        let worker = std::thread::Builder::new()
            .name("files-source-leases".into())
            .spawn(move || {
                // Declare before the lock so retirement is acknowledged only
                // after releasing history ownership, including unwinding.
                let _exit = WorkerExit(worker_lifetime);
                let _lock = lock;
                release_worker(reads, receive);
            })?;
        Ok(Self {
            sender: Some(send),
            lifetime,
            worker: Some(worker),
        })
    }

    fn flush(&self) -> io::Result<()> {
        let (send, receive) = mpsc::sync_channel(1);
        self.sender
            .as_ref()
            .unwrap()
            .send(Command::Flush(send))
            .map_err(|_| stopped())?;
        receive.recv().map_err(|_| stopped())?
    }

    fn owner(&self, record: requests::RequestRecord) -> RetainedSourceLease {
        self.lifetime.owners.fetch_add(1, Ordering::Relaxed);
        RetainedSourceLease {
            history: record.history,
            owner: Arc::new(Lease {
                id: Some(record.id()),
                sender: self.sender.as_ref().unwrap().clone(),
                lifetime: self.lifetime.clone(),
            }),
        }
    }
}

impl Drop for Releaser {
    fn drop(&mut self) {
        self.lifetime.closed.store(true, Ordering::Release);
        self.sender.take();
        if self.lifetime.owners.load(Ordering::Acquire) == 0
            && let Some(worker) = self.worker.take()
        {
            let _ = worker.join();
        }
        // Outstanding leases keep the worker and its shared history ownership
        // alive. Their final drop closes the channel; the detached worker drains.
    }
}

fn stopped() -> io::Error {
    io::Error::new(
        io::ErrorKind::BrokenPipe,
        "The retained-source release worker stopped. Restart Files before selecting more saved data.",
    )
}

fn retry_pending(reads: &ReadSource, pending: &mut BTreeSet<u64>) -> io::Result<()> {
    while let Some(id) = pending.first().copied() {
        reads.release_source_lease(id)?;
        pending.remove(&id);
    }
    Ok(())
}

fn release_worker(reads: ReadSource, receive: mpsc::Receiver<Command>) {
    let mut pending = BTreeSet::new();
    for command in receive {
        match command {
            Command::Release(id, reply) => {
                let result = reads.release_source_lease(id);
                if result.is_err() {
                    pending.insert(id);
                } else {
                    pending.remove(&id);
                }
                if let Some(reply) = reply {
                    let _ = reply.send(result);
                }
            }
            Command::Flush(reply) => {
                let _ = reply.send(retry_pending(&reads, &mut pending));
            }
        }
    }
    let _ = retry_pending(&reads, &mut pending);
}

impl OperationEngine {
    /// Worker-only. Protect captured retained data before publishing a process-
    /// owned selection. Acquires against the same journal ordering as requests;
    /// a stale or foreign capture cannot establish new ownership.
    pub fn lease_retained_sources(
        &self,
        sources: &RetainedSources,
    ) -> io::Result<RetainedSourceLease> {
        let mut releaser = self
            .source_leases
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if releaser.is_none() {
            *releaser = Some(Releaser::start(self.reads.clone(), self._lock.clone())?);
        }
        let releaser = releaser.as_ref().unwrap();
        releaser.flush()?;
        let record = self.reads.lease_retained_sources(sources)?;
        Ok(releaser.owner(record))
    }
}
