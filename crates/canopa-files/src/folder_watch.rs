//! Observation of the folders a frontend currently displays. A worker owns one
//! inotify instance, coalesces event bursts and publishes the affected folders;
//! the frontend re-reads its own snapshots. Watching grants no authority over
//! files and never interrupts accepted operations.
use canopa_file::watch::{Target, Watcher};
use futures_channel::mpsc;
use rustix::event::{EventfdFlags, PollFd, PollFlags, Timespec, eventfd, poll};
use std::{
    collections::BTreeSet,
    io,
    os::fd::OwnedFd,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// The shared watcher's target budget. Further folders stay manual (Refresh).
pub const MAXIMUM_FOLDERS: usize = 256;

/// Debounce policy: publish after a quiet period, or after the maximum delay
/// while a burst continues, so a long copy still refreshes periodically.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    pub quiet: Duration,
    pub maximum: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            quiet: Duration::from_millis(200),
            maximum: Duration::from_secs(1),
        }
    }
}

/// Folders whose contents may have changed since the previous call.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub folders: BTreeSet<PathBuf>,
    /// Some displayed folders are not observed: the kernel's watch budget or
    /// this watcher's folder budget is exhausted. Refresh still works.
    pub limited: bool,
}

struct Shared {
    desired: Mutex<Option<BTreeSet<PathBuf>>>,
    changed: Mutex<BTreeSet<PathBuf>>,
    limited: AtomicBool,
    stopped: AtomicBool,
    wake: OwnedFd,
    notice: Mutex<mpsc::Sender<()>>,
}

impl Shared {
    fn wake(&self) {
        let _ = rustix::io::write(&self.wake, &1u64.to_ne_bytes());
    }

    fn notify(&self) {
        let _ = self
            .notice
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_send(());
    }
}

struct Handle {
    shared: Arc<Shared>,
    notices: Mutex<Option<mpsc::Receiver<()>>>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.shared.stopped.store(true, Ordering::Release);
        self.shared.wake();
    }
}

#[derive(Clone)]
pub struct FolderWatch(Arc<Handle>);

impl FolderWatch {
    /// Fails when no inotify instance is available; callers keep Refresh.
    pub fn start(timing: Timing) -> io::Result<Self> {
        let watcher = Watcher::new(Vec::new())?;
        let (notice, notices) = mpsc::channel(1);
        let shared = Arc::new(Shared {
            desired: Mutex::new(None),
            changed: Mutex::new(BTreeSet::new()),
            limited: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            wake: eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?,
            notice: Mutex::new(notice),
        });
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("files-folder-watch".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(&worker, watcher, timing)
                }));
                if !matches!(result, Ok(Ok(()))) {
                    // Without observation every folder is effectively unwatched.
                    worker.limited.store(true, Ordering::Release);
                    eprintln!(
                        "canopa-files: automatic folder refresh stopped; use Refresh (F5) to update folders"
                    );
                    worker.notify();
                }
            })?;
        Ok(Self(Arc::new(Handle {
            shared,
            notices: Mutex::new(Some(notices)),
        })))
    }

    /// Replace the observed set. Folders that are no longer displayed stop
    /// being watched and their pending changes are discarded.
    pub fn watch(&self, folders: impl IntoIterator<Item = PathBuf>) {
        *self
            .0
            .shared
            .desired
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(folders.into_iter().collect());
        self.0.shared.wake();
    }

    pub fn take_notices(&self) -> Option<mpsc::Receiver<()>> {
        self.0
            .notices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    pub fn take_changes(&self) -> Changes {
        Changes {
            folders: std::mem::take(
                &mut *self
                    .0
                    .shared
                    .changed
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()),
            ),
            limited: self.limited(),
        }
    }

    pub fn limited(&self) -> bool {
        self.0.shared.limited.load(Ordering::Acquire)
    }
}

fn run(shared: &Shared, mut watcher: Watcher, timing: Timing) -> io::Result<()> {
    let mut watched = BTreeSet::new();
    let mut pending = BTreeSet::new();
    let mut burst: Option<(Instant, Instant)> = None;
    let mut over_budget = false;
    let mut logged = false;
    while !shared.stopped.load(Ordering::Acquire) {
        let now = Instant::now();
        let timeout = if !watcher.drained() {
            Some(Duration::ZERO)
        } else {
            burst.map(|(first, last)| {
                (last + timing.quiet)
                    .min(first + timing.maximum)
                    .saturating_duration_since(now)
            })
        };
        let timeout = timeout.map(|timeout| Timespec {
            tv_sec: timeout.as_secs() as _,
            tv_nsec: timeout.subsec_nanos() as _,
        });
        let mut descriptors = [
            PollFd::new(&watcher, PollFlags::IN),
            PollFd::new(&shared.wake, PollFlags::IN),
        ];
        match poll(&mut descriptors, timeout.as_ref()) {
            Ok(_) | Err(rustix::io::Errno::INTR) => {}
            Err(error) => return Err(error.into()),
        }
        let events = !descriptors[0].revents().is_empty();
        if !descriptors[1].revents().is_empty() {
            let mut counter = [0; 8];
            let _ = rustix::io::read(&shared.wake, &mut counter);
        }
        if shared.stopped.load(Ordering::Acquire) {
            break;
        }
        let desired = shared
            .desired
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(desired) = desired {
            let mut folders: Vec<_> = desired
                .into_iter()
                .filter(|path| path.is_absolute() && path.as_os_str().len() <= 4096)
                .collect();
            over_budget = folders.len() > MAXIMUM_FOLDERS;
            folders.truncate(MAXIMUM_FOLDERS);
            watcher.set_targets(folders.iter().cloned().map(Target::Directory).collect())?;
            watched = folders.into_iter().collect();
            pending.retain(|path| watched.contains(path));
            if pending.is_empty() {
                burst = None;
            }
        }
        if events || !watcher.drained() {
            let mut any = false;
            for target in watcher.changed_targets()? {
                let (Target::Directory(path) | Target::File(path)) = target;
                if watched.contains(&path) {
                    pending.insert(path);
                    any = true;
                }
            }
            if any {
                let now = Instant::now();
                burst = Some(burst.map_or((now, now), |(first, _)| (first, now)));
            }
        }
        let limited = over_budget || watcher.limited();
        if shared.limited.swap(limited, Ordering::AcqRel) != limited {
            if limited && !logged {
                logged = true;
                eprintln!(
                    "canopa-files: the inotify watch limit is reached; some folders will not refresh automatically. Use Refresh (F5), or raise fs.inotify.max_user_watches."
                );
            }
            shared.notify();
        }
        let now = Instant::now();
        if let Some((first, last)) = burst
            && (now >= last + timing.quiet || now >= first + timing.maximum)
        {
            burst = None;
            shared
                .changed
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .append(&mut pending);
            shared.notify();
        }
    }
    Ok(())
}
