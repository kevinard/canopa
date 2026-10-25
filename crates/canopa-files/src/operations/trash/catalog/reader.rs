//! One observed, coalescing catalog reader, hostable without a renderer.
use super::*;
use futures_channel::mpsc;
use futures_util::StreamExt;
use gio::prelude::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug)]
pub struct Published {
    pub revision: u64,
    pub scope: Scope,
    pub loading: bool,
    pub catalog: Arc<Catalog>,
    pub error: Option<String>,
}

struct State {
    published: Published,
    cancellation: Cancellation,
}

struct Shared {
    data_home: PathBuf,
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

    fn request(&self, scope: Option<Scope>) {
        if self.stopped.load(Ordering::Acquire) {
            return;
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let Some(revision) = state.published.revision.checked_add(1) else {
            state.published.error =
                Some("Trash catalog identities exhausted. Reopen Trash.".into());
            state.published.loading = false;
            drop(state);
            self.notify();
            return;
        };
        if let Some(scope) = scope.filter(|scope| *scope != state.published.scope) {
            state.published.scope = scope;
            state.published.catalog = Arc::new(Catalog::default());
        }
        state.cancellation.cancel();
        state.cancellation = Cancellation::default();
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

    fn failed(&self, message: String) {
        self.stopped.store(true, Ordering::Release);
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.published.loading = false;
        state.published.error = Some(message);
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

/// Read-only catalog lifetime. Accepted operations use OperationJobs separately.
#[derive(Clone)]
pub struct Reader(Arc<Handle>);

impl Reader {
    pub fn start(data_home: PathBuf, scope: Scope) -> io::Result<Self> {
        TrashLocation::home(&data_home)?;
        let (wake, receiver) = mpsc::channel(1);
        let (notice, notices) = mpsc::channel(1);
        let shared = Arc::new(Shared {
            data_home,
            state: Mutex::new(State {
                published: Published {
                    revision: 1,
                    scope,
                    loading: true,
                    catalog: Arc::new(Catalog::default()),
                    error: None,
                },
                cancellation: Cancellation::default(),
            }),
            wake: Mutex::new(wake),
            notice: Mutex::new(notice),
            stopped: AtomicBool::new(false),
        });
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("files-trash-catalog".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(worker.clone(), receiver)
                }));
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => worker.failed(error.to_string()),
                    Err(_) => {
                        worker.failed("The Trash reader stopped. Close and reopen Trash.".into())
                    }
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

    pub fn set_scope(&self, scope: Scope) {
        self.0.shared.request(Some(scope));
    }
    pub fn refresh(&self) {
        self.0.shared.request(None);
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
    paths: BTreeSet<PathBuf>,
    monitors: Vec<gio::FileMonitor>,
    warnings: Vec<String>,
}

impl Watches {
    fn prepare(&mut self, discovery: Option<&LocationDiscovery>, shared: &Arc<Shared>) {
        let home = shared.data_home.join("Trash");
        let roots = discovery.map_or(&[][..], |discovery| discovery.locations.as_slice());
        let paths = roots
            .iter()
            .map(|root| root.directory())
            .chain(std::iter::once(home.as_path()))
            .flat_map(|root| [root.to_owned(), root.join("files"), root.join("info")])
            .chain(
                discovery
                    .into_iter()
                    .flat_map(|discovery| discovery.creation_parents.iter().cloned()),
            )
            .filter_map(|path| {
                path.ancestors()
                    .find(|path| path.is_dir())
                    .map(Path::to_owned)
            })
            .collect::<BTreeSet<_>>();
        if self.paths == paths && self.warnings.is_empty() {
            return;
        }
        for monitor in self.monitors.drain(..) {
            monitor.cancel();
        }
        self.paths = paths;
        self.warnings.clear();
        for path in &self.paths {
            match gio::File::for_path(path)
                .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
            {
                Ok(monitor) => {
                    monitor.set_rate_limit(150);
                    let shared = shared.clone();
                    monitor.connect_changed(move |_, _, _, _| shared.request(None));
                    self.monitors.push(monitor);
                }
                Err(error) => self.warnings.push(format!(
                    "Automatic refresh unavailable for {}: {error}. Use Refresh.",
                    path.display()
                )),
            }
        }
    }
}

impl Drop for Watches {
    fn drop(&mut self) {
        for monitor in &self.monitors {
            monitor.cancel();
        }
    }
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
                    let (revision, scope, cancellation) = {
                        let state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
                        (
                            state.published.revision,
                            state.published.scope,
                            state.cancellation.clone(),
                        )
                    };
                    let locations = locations(&shared.data_home, scope);
                    // Observe before reading: a move between reading and installing
                    // a monitor must not leave a permanently stale empty catalog.
                    watches.prepare(locations.as_ref().ok(), &shared);
                    let result = locations.and_then(|discovery| {
                        Catalog::read_locations(discovery.locations, &cancellation)
                    });
                    let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
                    if state.published.revision != revision || cancellation.is_cancelled() {
                        continue;
                    }
                    state.published.loading = false;
                    match result {
                        Ok(mut catalog) => {
                            catalog.warnings.extend(watches.warnings.clone());
                            state.published.catalog = Arc::new(catalog);
                            state.published.error = None;
                        }
                        Err(error) => {
                            state.published.catalog = Arc::new(Catalog::default());
                            state.published.error = Some(error.to_string());
                        }
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
