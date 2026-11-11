//! Batched durability barriers for new-item Copy (ADR0165).
//!
//! A Copy transfer opens one [`Session`]. Inside [`Store::deferred`] spans, the
//! plain new-item Copy steps keep their exact journal/filesystem order but skip
//! their per-step barriers: journal commits use SQLite `synchronous=NORMAL`,
//! and payload/folder syncs only remember the destination filesystem. The
//! session's [`Session::flush`] then runs one `syncfs` per destination filesystem
//! followed by one journal sync. Every other step, and every span outside a
//! session, still syncs immediately with `synchronous=FULL`.
//!
//! Process interruption is unaffected: every write still reaches the kernel in
//! the original order before the next step. Only power loss or a kernel crash
//! inside an unflushed batch can lose that batch's new copies or history rows;
//! sources and existing destination data are never part of a deferred step.
//!
//! State is thread-local because one transfer runs synchronously on its
//! worker thread; nested or concurrent sessions on other threads are separate.
use std::{
    cell::{Cell, RefCell},
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Flush at least this often, bounding the power-loss window and keeping
/// reported progress close to what the device has actually accepted.
const MAX_AGE: Duration = Duration::from_secs(1);
/// Unsynced content bytes before a flush. Also the in-file write-behind window.
pub(super) const MAX_BYTES: u64 = 32 * 1024 * 1024;

thread_local! {
    static SESSION: RefCell<Option<Batch>> = const { RefCell::new(None) };
    static DEFERRING: Cell<bool> = const { Cell::new(false) };
    /// Deferred syncs and batch barriers on this thread, for tests.
    #[cfg(test)]
    static COUNTS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

#[derive(Default)]
struct Batch {
    journal: Vec<PathBuf>,
    /// One open descriptor per destination filesystem, by device number.
    filesystems: Vec<(u64, fs::File)>,
    items: usize,
    bytes: u64,
    since: Option<Instant>,
}

/// One Copy transfer's barrier batch. Dropping without [`Session::flush`]
/// still syncs on a best-effort basis; callers flush to observe errors.
pub(super) struct Session {
    active: bool,
}

impl Session {
    /// Begin batching on this thread. `journal` lists the history files whose
    /// deferred commits the final barrier must sync. A nested call is inert.
    pub(super) fn begin(journal: Vec<PathBuf>) -> Self {
        SESSION.with(|session| {
            let mut session = session.borrow_mut();
            if session.is_some() {
                return Self { active: false };
            }
            *session = Some(Batch {
                journal,
                ..Batch::default()
            });
            Self { active: true }
        })
    }

    /// Make every deferred step so far durable. Ends the session.
    pub(super) fn flush(mut self) -> io::Result<()> {
        let result = if self.active { flush() } else { Ok(()) };
        self.end();
        result
    }

    fn end(&mut self) {
        if std::mem::take(&mut self.active) {
            SESSION.with(|session| session.borrow_mut().take());
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.active {
            let _ = flush();
            self.end();
        }
    }
}

/// Whether a session is batching on this thread.
pub(super) fn batching() -> bool {
    SESSION.with(|session| session.borrow().is_some())
}

/// Whether the current step may defer its barrier.
pub(super) fn deferring() -> bool {
    DEFERRING.get()
}

/// Mark the current thread as inside a deferred span until the guard drops.
pub(super) fn enter() -> Option<impl Drop> {
    struct Leave;
    impl Drop for Leave {
        fn drop(&mut self) {
            DEFERRING.set(false);
        }
    }
    if !batching() || DEFERRING.get() {
        return None;
    }
    DEFERRING.set(true);
    // A span's commits are unsynced until the next barrier.
    SESSION.with(|session| {
        if let Some(batch) = session.borrow_mut().as_mut() {
            batch.since.get_or_insert_with(Instant::now);
        }
    });
    Some(Leave)
}

/// `sync_all`, or remember the file's filesystem for the batch barrier.
pub(super) fn sync(file: &fs::File) -> io::Result<()> {
    if !deferring() {
        return file.sync_all();
    }
    remember(file)
}

/// Sync a path's parent folder, or defer it like [`sync`].
pub(super) fn sync_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing parent"))?;
    sync(&fs::File::open(parent)?)
}

fn remember(file: &fs::File) -> io::Result<()> {
    #[cfg(test)]
    COUNTS.set((COUNTS.get().0 + 1, COUNTS.get().1));
    let device = file.metadata()?.dev();
    SESSION.with(|session| {
        let mut session = session.borrow_mut();
        let batch = session
            .as_mut()
            .ok_or_else(|| io::Error::other("a deferred sync needs its batch"))?;
        batch.since.get_or_insert_with(Instant::now);
        if !batch.filesystems.iter().any(|(known, _)| *known == device) {
            // A descriptor opened now samples the filesystem's error sequence,
            // so syncfs reports writeback failures of the data written after it.
            batch.filesystems.push((device, file.try_clone()?));
        }
        Ok(())
    })
}

/// Account one completed deferred item and flush when the batch is due.
pub(super) fn completed(bytes: u64) -> io::Result<()> {
    let due = SESSION.with(|session| {
        let mut session = session.borrow_mut();
        let Some(batch) = session.as_mut() else {
            return false;
        };
        batch.items += 1;
        batch.bytes = batch.bytes.saturating_add(bytes);
        batch.bytes >= MAX_BYTES || batch.since.is_some_and(|since| since.elapsed() >= MAX_AGE)
    });
    if due { flush() } else { Ok(()) }
}

/// Make every deferred step so far durable: filesystems first, then the
/// journal, so a durable history row never precedes its own barrier.
pub(super) fn flush() -> io::Result<()> {
    // A batch with nothing deferred since its last barrier is already durable.
    let taken = SESSION.with(|session| {
        let mut session = session.borrow_mut();
        session
            .as_mut()
            .filter(|batch| batch.since.is_some())
            .map(|batch| {
                batch.items = 0;
                batch.bytes = 0;
                batch.since = None;
                (
                    std::mem::take(&mut batch.filesystems),
                    batch.journal.clone(),
                )
            })
    });
    let Some((filesystems, journal)) = taken else {
        return Ok(());
    };
    #[cfg(test)]
    COUNTS.set((COUNTS.get().0, COUNTS.get().1 + 1));
    for (_, file) in &filesystems {
        rustix::fs::syncfs(file)?;
    }
    for path in journal {
        match fs::File::open(&path) {
            Ok(file) => file.sync_all()?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Deferred syncs and batch barriers on this thread since the last call.
#[cfg(test)]
pub(super) fn take_counts() -> (usize, usize) {
    COUNTS.replace((0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_defer_only_inside_a_session_and_flush_on_demand() {
        take_counts();
        let root = tempfile::tempdir().unwrap();
        let file = fs::File::create(root.path().join("payload")).unwrap();
        assert!(enter().is_none());
        sync(&file).unwrap();
        let session = Session::begin(vec![root.path().join("missing-wal")]);
        assert!(Session::begin(Vec::new()).flush().is_ok());
        assert!(batching());
        {
            let _span = enter().unwrap();
            assert!(deferring());
            assert!(enter().is_none());
            sync(&file).unwrap();
            sync_parent(&root.path().join("payload")).unwrap();
            completed(1).unwrap();
        }
        assert!(!deferring());
        assert_eq!(take_counts(), (2, 0));
        completed(MAX_BYTES).unwrap();
        assert_eq!(take_counts(), (0, 1));
        session.flush().unwrap();
        assert!(!batching());
        assert_eq!(take_counts(), (0, 0), "nothing was deferred since");
    }

    #[test]
    fn copy_batches_its_barriers_and_move_keeps_per_step_syncs() {
        use crate::{
            Cancellation,
            operations::OperationEngine,
            transfers::{TransferKind, TransferReply, TransferRequest},
        };
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::create_dir_all(source.join("nested")).unwrap();
        for n in 0..20 {
            fs::write(source.join(format!("{n:02}")), [n as u8; 100]).unwrap();
        }
        fs::write(source.join("nested/leaf"), b"leaf").unwrap();
        let output = root.path().join("output");
        let moved = root.path().join("moved");
        fs::create_dir(&output).unwrap();
        fs::create_dir(&moved).unwrap();
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        let transfer = |kind, sources, destination: &Path| {
            engine
                .transfer(
                    &TransferRequest {
                        kind,
                        sources,
                        destination: destination.to_owned(),
                    },
                    &Cancellation::default(),
                    |_| TransferReply::Continue,
                )
                .unwrap()
        };
        take_counts();
        let copied = transfer(TransferKind::Copy, vec![source.clone()], &output);
        assert_eq!(copied.completed, 23);
        let (deferred, barriers) = take_counts();
        // Payload, stage and parent syncs for 21 files and 2 folders.
        assert!(deferred >= 3 * 21, "{deferred} deferred syncs");
        assert!((1..=3).contains(&barriers), "{barriers} barriers");
        assert!(!batching() && !deferring());
        assert_eq!(
            fs::read(output.join("source/nested/leaf")).unwrap(),
            b"leaf"
        );
        let moved_result = transfer(TransferKind::Move, vec![output.join("source")], &moved);
        assert_eq!(moved_result.completed, 1);
        assert_eq!(take_counts(), (0, 0), "Move never defers");
        engine.undo(&Cancellation::default()).unwrap();
        engine.undo(&Cancellation::default()).unwrap();
        assert!(fs::read_dir(&output).unwrap().next().is_none());
        assert_eq!(fs::read_dir(&source).unwrap().count(), 21);
    }
}
