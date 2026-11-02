//! Event-driven session and selected-avatar source refresh. No idle rescanning.
use crate::config::{Config, Session};
use canopa_file::watch::{Target, Watcher};
use std::{path::PathBuf, time::Duration};
#[derive(Clone, Default)]
struct Request {
    avatars: Vec<PathBuf>,
    revision: u64,
}
pub struct Worker {
    request: tokio::sync::watch::Sender<Request>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(
        mut config: Config,
        mut notify: impl FnMut(Vec<Session>, bool) + Send + 'static,
    ) -> Self {
        let (request, mut pending) = tokio::sync::watch::channel(Request::default());
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            runtime.block_on(async {
                let Ok(watcher)=Watcher::new(targets(&config,&[])) else{return;};
                let Ok(mut watcher)=tokio::io::unix::AsyncFd::new(watcher) else{return;};
                config.resolve_sessions(config.system_sources);
                notify(config.sessions.clone(),false);
                let mut last_revision=0;
                loop {
                    let refresh=tokio::select! {biased;
                        _=&mut stopped=>return,
                        changed=pending.changed()=> {
                            if changed.is_err(){return;}
                            let request=pending.borrow_and_update().clone();
                            let _=watcher.get_mut().set_targets(targets(&config,&request.avatars));
                            // Path changes alone do not rescan catalogs. A revision
                            // is the explicit user refresh action (F5).
                            let refresh=request.revision!=last_revision;last_revision=request.revision;refresh
                        },
                        ready=watcher.readable_mut()=> {
                            let Ok(mut ready)=ready else{return;};
                            let changed=ready.get_inner_mut().changed().unwrap_or(false);
                            if ready.get_inner().drained() {ready.clear_ready();}
                            if !changed {continue;}
                            true
                        }
                    };
                    if !refresh {continue;}
                    // Coalesce a burst into one bounded admission scan, without
                    // adding any periodic work while the sources are unchanged.
                    tokio::select! {_=&mut stopped=>return,_=tokio::time::sleep(Duration::from_millis(100))=>{}}
                    config.resolve_sessions(config.system_sources);
                    let request=pending.borrow_and_update().clone();
                    let _=watcher.get_mut().set_targets(targets(&config,&request.avatars));
                    notify(config.sessions.clone(),true);
                }
            });
        });
        Self {
            request,
            stop: Some(stop),
            thread: Some(thread),
        }
    }
    pub fn avatars(&self, paths: Vec<PathBuf>) {
        self.request.send_modify(|r| r.avatars = paths);
    }
    pub fn refresh(&self) {
        self.request
            .send_modify(|r| r.revision = r.revision.wrapping_add(1).max(1));
    }
}
fn targets(config: &Config, avatars: &[PathBuf]) -> Vec<Target> {
    let mut targets = Vec::new();
    if config.discovery.enabled {
        targets.extend(
            config
                .discovery
                .wayland_dirs
                .iter()
                .cloned()
                .map(Target::Directory),
        );
        if config.discovery.x11_wrapper.is_some() {
            targets.extend(
                config
                    .discovery
                    .x11_dirs
                    .iter()
                    .cloned()
                    .map(Target::Directory),
            );
        }
        targets.extend(
            [
                "/usr/local/bin",
                "/usr/bin",
                "/bin",
                "/usr/local/sbin",
                "/usr/sbin",
                "/sbin",
            ]
            .map(|p| Target::Directory(p.into())),
        );
        targets.extend(
            config
                .sessions
                .iter()
                .filter_map(|s| s.argv.first())
                .filter(|p| PathBuf::from(p).is_absolute())
                .map(|p| Target::File(p.into())),
        );
    }
    targets.extend(avatars.iter().cloned().map(Target::File));
    targets.truncate(256);
    targets
}
impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
