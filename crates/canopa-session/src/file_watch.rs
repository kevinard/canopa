//! One sleeping inotify observer; admission acknowledgements coalesce source events.
use canopa_file::watch::{Target, Watcher};
use std::{os::unix::fs::MetadataExt, path::PathBuf, time::Duration};
#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    path: PathBuf,
    metadata: Option<(u64, u64, u64, i64, i64, i64, i64)>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Snapshot(Vec<Stamp>);
pub fn retain_snapshot(baseline: &Snapshot, targets: &[Target]) -> Snapshot {
    Snapshot(
        targets
            .iter()
            .map(|target| {
                let (Target::File(path) | Target::Directory(path)) = target;
                baseline
                    .0
                    .iter()
                    .find(|stamp| &stamp.path == path)
                    .cloned()
                    .unwrap_or_else(|| snapshot(std::slice::from_ref(target)).0.remove(0))
            })
            .collect(),
    )
}
pub fn snapshot(targets: &[Target]) -> Snapshot {
    Snapshot(
        targets
            .iter()
            .map(|target| {
                let (Target::File(path) | Target::Directory(path)) = target;
                let metadata = path.metadata().ok().map(|m| {
                    (
                        m.dev(),
                        m.ino(),
                        m.len(),
                        m.mtime(),
                        m.mtime_nsec(),
                        m.ctime(),
                        m.ctime_nsec(),
                    )
                });
                Stamp {
                    path: path.clone(),
                    metadata,
                }
            })
            .collect(),
    )
}
#[derive(Clone)]
struct Request {
    targets: Vec<Target>,
    baseline: Snapshot,
}
pub struct Observer {
    request: tokio::sync::watch::Sender<Request>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Observer {
    pub fn start(targets: Vec<Target>, notify: impl Fn() + Send + 'static) -> Self {
        let baseline = snapshot(&targets);
        let (request, mut pending) = tokio::sync::watch::channel(Request { targets, baseline });
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            runtime.block_on(async {
                let mut current=pending.borrow_and_update().clone();
                let Ok(watcher)=Watcher::new(current.targets.clone()) else{return;};
                // SAFETY: Watcher owns its inotify descriptor and never replaces it.
                let Ok(mut watcher)=(unsafe{tokio::io::unix::AsyncFd::register(watcher)}) else{return;};
                // One startup reconciliation closes the initial read/watch gap.
                notify();let mut notified=true;
                loop {
                    tokio::select! {biased;
                        _=&mut stopped=>return,
                        changed=pending.changed()=>{
                            if changed.is_err(){return;}
                            current=pending.borrow_and_update().clone();
                            let _=watcher.get_mut().set_targets(current.targets.clone());
                            notified=false;
                            // Admission captured this baseline before reading source
                            // bytes. A change during admission must be tried again.
                            if snapshot(&current.targets)!=current.baseline {notify();notified=true;}
                        },
                        ready=watcher.readable_mut()=>{
                            let Ok(mut ready)=ready else{return;};
                            let changed=ready.get_inner_mut().changed().unwrap_or(false);
                            if ready.get_inner().drained(){ready.clear_ready();}
                            if !changed || notified {continue;}
                            tokio::select! {_=&mut stopped=>return,_=tokio::time::sleep(Duration::from_millis(100))=>{}}
                            // Ignore events already covered by an admitted snapshot.
                            if snapshot(&current.targets)!=current.baseline {notify();notified=true;}
                        }
                    }
                }
            });
        });
        Self {
            request,
            stop: Some(stop),
            thread: Some(thread),
        }
    }
    pub fn set(&self, targets: Vec<Target>) {
        let baseline = snapshot(&targets);
        self.admitted(targets, baseline);
    }
    pub fn admitted(&self, targets: Vec<Target>, baseline: Snapshot) {
        self.request.send_replace(Request { targets, baseline });
    }
}
impl Drop for Observer {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admission_ack_coalesces_events_and_retries_changes_during_read_or_after_rejection() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("source");
        std::fs::write(&path, b"one").unwrap();
        let targets = vec![Target::File(path.clone())];
        let (sender, receiver) = std::sync::mpsc::channel();
        let observer = Observer::start(targets.clone(), move || {
            let _ = sender.send(());
        });
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        observer.admitted(targets.clone(), snapshot(&targets));
        assert!(receiver.recv_timeout(Duration::from_millis(150)).is_err());
        std::fs::write(&path, b"changed-source").unwrap();
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        let before_read = snapshot(&targets);
        std::fs::write(&path, b"changed-again-during-admission").unwrap();
        observer.admitted(targets.clone(), before_read);
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        observer.admitted(targets.clone(), snapshot(&targets));
        assert!(
            receiver.recv_timeout(Duration::from_millis(250)).is_err(),
            "an admitted event caused a redundant decode"
        );
        std::fs::write(&path, b"rejected").unwrap();
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        observer.admitted(targets.clone(), snapshot(&targets));
        std::fs::write(&path, b"fresh-valid-source").unwrap();
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        let before = std::time::Instant::now();
        drop(observer);
        assert!(before.elapsed() < Duration::from_millis(500));
    }
}
