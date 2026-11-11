//! Observed Recovery contents, read through the process's existing queue owner.
use crate::{
    Cancellation,
    operation_jobs::OperationJobs,
    operations::{InterruptedOperation, RecoveryItem},
};
use futures_channel::mpsc;
use futures_util::StreamExt;
use gio::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    io,
    os::unix::fs::MetadataExt,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub items: Vec<RecoveryItem>,
    pub interrupted: Vec<InterruptedOperation>,
    pub has_more: bool,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Published {
    pub revision: u64,
    pub loading: bool,
    pub catalog: Arc<Catalog>,
    pub error: Option<String>,
}

struct State {
    published: Published,
    pages: usize,
    cancellation: Cancellation,
}
struct Shared {
    jobs: OperationJobs,
    state: Mutex<State>,
    wake: Mutex<mpsc::Sender<()>>,
    notice: Mutex<mpsc::Sender<()>>,
    stopped: AtomicBool,
}
impl Shared {
    fn notify(&self) {
        let _ = self
            .notice
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_send(());
    }
    fn request(&self, more: bool) {
        if self.stopped.load(Ordering::Acquire) {
            return;
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if more && (state.published.loading || !state.published.catalog.has_more) {
            return;
        }
        let next = state
            .published
            .revision
            .checked_add(1)
            .zip(state.pages.checked_add(usize::from(more)));
        let Some((revision, pages)) = next else {
            state.published.error = Some(fl!("files-recovery-identities-exhausted"));
            state.published.loading = false;
            drop(state);
            self.notify();
            return;
        };
        state.cancellation.cancel();
        state.cancellation = Cancellation::default();
        state.pages = pages;
        state.published.revision = revision;
        state.published.loading = true;
        state.published.error = None;
        drop(state);
        self.notify();
        let _ = self
            .wake
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_send(());
    }
    fn failed(&self, error: String) {
        self.stopped.store(true, Ordering::Release);
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.cancellation.cancel();
        state.published.loading = false;
        state.published.error = Some(error);
        drop(state);
        self.notify();
    }
}
struct Handle {
    shared: Arc<Shared>,
    notices: Mutex<Option<mpsc::Receiver<()>>>,
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.shared.stopped.store(true, Ordering::Release);
        self.shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .cancellation
            .cancel();
        let _ = self
            .shared
            .wake
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_send(());
    }
}

/// One active preparation and one coalesced pending refresh. Closing the reader
/// cancels inspection only; admitted operations retain their own lifetime.
#[derive(Clone)]
pub struct Reader(Arc<Handle>);
impl Reader {
    pub fn start(jobs: OperationJobs) -> io::Result<Self> {
        let (wake, receiver) = mpsc::channel(1);
        let (notice, notices) = mpsc::channel(1);
        let shared = Arc::new(Shared {
            jobs,
            state: Mutex::new(State {
                published: Published {
                    revision: 1,
                    loading: true,
                    catalog: Arc::new(Catalog::default()),
                    error: None,
                },
                pages: 1,
                cancellation: Cancellation::default(),
            }),
            wake: Mutex::new(wake),
            notice: Mutex::new(notice),
            stopped: AtomicBool::new(false),
        });
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("files-recovery-catalog".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(worker.clone(), receiver)
                }));
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => worker.failed(error.to_string()),
                    Err(_) => worker.failed(fl!("files-recovery-reader-stopped-reopen")),
                }
            })?;
        let _ = shared
            .wake
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_send(());
        Ok(Self(Arc::new(Handle {
            shared,
            notices: Mutex::new(Some(notices)),
        })))
    }
    pub fn snapshot(&self) -> Published {
        self.0
            .shared
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .published
            .clone()
    }
    pub fn refresh(&self) {
        self.0.shared.request(false);
    }
    pub fn load_more(&self) {
        self.0.shared.request(true);
    }
    pub fn take_notices(&self) -> Option<mpsc::Receiver<()>> {
        self.0
            .notices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }
}

#[derive(Default)]
struct Watches {
    directories: BTreeMap<PathBuf, WatchedDirectory>,
    active: BTreeSet<PathBuf>,
    monitors: Vec<gio::FileMonitor>,
    warnings: Vec<String>,
    pending: Arc<Mutex<Option<glib::Source>>>,
}
#[derive(PartialEq, Eq)]
struct WatchedDirectory {
    device: u64,
    inode: u64,
    targets: BTreeSet<PathBuf>,
}
impl Watches {
    /// Install replacements before retiring old monitors. New coverage requires
    /// another read, closing the discovery-to-observation gap.
    fn prepare(&mut self, targets: BTreeSet<PathBuf>, shared: &Arc<Shared>) -> bool {
        let mut directories = BTreeMap::<PathBuf, WatchedDirectory>::new();
        for target in &targets {
            let directory = std::fs::symlink_metadata(target).is_ok_and(|m| m.is_dir());
            let paths = target
                .parent()
                .into_iter()
                .chain(directory.then_some(target.as_path()));
            for path in paths {
                if let Some((existing, metadata)) = path.ancestors().find_map(|p| {
                    std::fs::metadata(p)
                        .ok()
                        .filter(|m| m.is_dir())
                        .map(|m| (p, m))
                }) {
                    directories
                        .entry(existing.to_owned())
                        .or_insert_with(|| WatchedDirectory {
                            device: metadata.dev(),
                            inode: metadata.ino(),
                            targets: BTreeSet::new(),
                        })
                        .targets
                        .insert(target.clone());
                }
            }
        }
        if directories == self.directories && self.warnings.is_empty() {
            return false;
        }
        let changed = directories != self.directories;
        let mut monitors = Vec::new();
        let mut active = BTreeSet::new();
        self.warnings.clear();
        for (path, directory) in &directories {
            match gio::File::for_path(path)
                .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
            {
                Ok(monitor) => {
                    monitor.set_rate_limit(150);
                    let shared = shared.clone();
                    let targets = directory.targets.clone();
                    let pending = self.pending.clone();
                    let context = glib::MainContext::ref_thread_default();
                    monitor.connect_changed(move |_, file, other, _| {
                        if std::iter::once(file)
                            .chain(other)
                            .filter_map(|file| file.path())
                            .any(|path| {
                                targets.iter().any(|target| {
                                    path.starts_with(target) || target.starts_with(&path)
                                })
                            })
                        {
                            // One bounded delay for the whole reader, including
                            // overlapping directory and WAL notifications.
                            let mut timer = pending.lock().unwrap_or_else(|e| e.into_inner());
                            if timer.is_none() {
                                let shared = shared.clone();
                                let pending = pending.clone();
                                let source = glib::timeout_source_new(
                                    std::time::Duration::from_millis(150),
                                    Some("files-recovery-refresh"),
                                    glib::Priority::DEFAULT,
                                    move || {
                                        pending.lock().unwrap_or_else(|e| e.into_inner()).take();
                                        shared.request(false);
                                        glib::ControlFlow::Break
                                    },
                                );
                                source.attach(Some(&context));
                                *timer = Some(source);
                            }
                        }
                    });
                    active.insert(path.clone());
                    monitors.push(monitor);
                }
                Err(error) => self.warnings.push(fl!(
                    "files-recovery-refresh-unavailable",
                    path = path.display().to_string(),
                    error = error.to_string()
                )),
            }
        }
        let added = active.iter().any(|path| !self.active.contains(path));
        for monitor in self.monitors.drain(..) {
            monitor.cancel();
        }
        self.monitors = monitors;
        self.directories = directories;
        self.active = active;
        changed || added
    }
}
impl Drop for Watches {
    fn drop(&mut self) {
        if let Some(source) = self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            source.destroy();
        }
        for monitor in &self.monitors {
            monitor.cancel();
        }
    }
}

async fn read(
    shared: &Shared,
    pages: usize,
    cancellation: &Cancellation,
) -> io::Result<(Catalog, BTreeSet<PathBuf>)> {
    let mut catalog = Catalog::default();
    let mut paths = BTreeSet::new();
    let mut seen = HashSet::new();
    let mut cursor = None;
    for _ in 0..pages {
        if cancellation.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                fl!("files-recovery-read-cancelled"),
            ));
        }
        let page = shared
            .jobs
            .recovery_page(cursor, cancellation.clone())?
            .await
            .map_err(|_| io::Error::other(fl!("files-recovery-queue-stopped")))?
            .map_err(io::Error::other)?;
        catalog.items.extend(
            page.items
                .into_iter()
                .filter(|item| seen.insert(item.identity())),
        );
        catalog.interrupted = page.interrupted;
        paths.extend(page.observation_paths);
        cursor = page.next;
        if cursor.is_none() {
            break;
        }
    }
    catalog.has_more = cursor.is_some();
    Ok((catalog, paths))
}

fn run(shared: Arc<Shared>, mut receiver: mpsc::Receiver<()>) -> io::Result<()> {
    let context = glib::MainContext::new();
    context
        .with_thread_default(|| {
            let main_loop = glib::MainLoop::new(Some(&context), false);
            let finish = main_loop.clone();
            context.spawn_local(async move {
                let mut watches = Watches::default();
                while receiver.next().await.is_some() {
                    if shared.stopped.load(Ordering::Acquire) {
                        break;
                    }
                    let (revision, pages, cancellation) = {
                        let state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
                        (
                            state.published.revision,
                            state.pages,
                            state.cancellation.clone(),
                        )
                    };
                    let result = loop {
                        let result = read(&shared, pages, &cancellation).await;
                        if let Ok((_, paths)) = &result
                            && watches.prepare(paths.clone(), &shared)
                            && !cancellation.is_cancelled()
                        {
                            continue;
                        }
                        break result;
                    };
                    let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
                    if state.published.revision != revision || cancellation.is_cancelled() {
                        continue;
                    }
                    state.published.loading = false;
                    match result {
                        Ok((mut catalog, _)) => {
                            catalog.warnings = watches.warnings.clone();
                            state.published.catalog = Arc::new(catalog);
                            state.published.error = None;
                        }
                        Err(error) => state.published.error = Some(error.to_string()),
                    }
                    drop(state);
                    shared.notify();
                }
                finish.quit();
            });
            main_loop.run();
        })
        .map_err(io::Error::other)
}
