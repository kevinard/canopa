//! A retained approval remembers policy denials, including coalesced toggles.
use crate::lockdown::{Property, Snapshot};
use tokio::sync::watch;

pub(super) struct Permit {
    initial: Snapshot,
    policy: watch::Receiver<Snapshot>,
    shutdown: watch::Receiver<bool>,
    file: bool,
    revoked: watch::Sender<bool>,
    monitor: tokio::task::JoinHandle<()>,
}
impl Permit {
    pub fn new(
        policy: watch::Receiver<Snapshot>,
        shutdown: watch::Receiver<bool>,
        initial: Snapshot,
        file: bool,
    ) -> Self {
        let (revoked, _) = watch::channel(false);
        let mut observed = policy.clone();
        let mut stopped = shutdown.clone();
        let signal = revoked.clone();
        let monitor = tokio::spawn(async move {
            loop {
                if !valid(&observed, &stopped, &initial, file) {
                    signal.send_replace(true);
                    break;
                }
                tokio::select! {
                    _ = observed.changed() => {},
                    _ = stopped.changed() => {},
                }
            }
        });
        Self {
            initial,
            policy,
            shutdown,
            file,
            revoked,
            monitor,
        }
    }
    pub fn current(&self) -> bool {
        if *self.revoked.borrow() || !valid(&self.policy, &self.shutdown, &self.initial, self.file)
        {
            self.revoked.send_replace(true);
            false
        } else {
            true
        }
    }
    pub fn cancelled(&self) -> watch::Receiver<bool> {
        self.revoked.subscribe()
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.revoked.send_replace(true);
        self.monitor.abort();
    }
}
fn valid(
    policy: &watch::Receiver<Snapshot>,
    shutdown: &watch::Receiver<bool>,
    initial: &Snapshot,
    file: bool,
) -> bool {
    policy.has_changed().is_ok()
        && shutdown.has_changed().is_ok()
        && !*shutdown.borrow()
        && policy.borrow().permits(initial, Property::Printing)
        && (!file || policy.borrow().permits(initial, Property::SaveToDisk))
}

/// Drop explicitly signals independently owned worker tasks before aborting the
/// monitor. Cancellation of the method future therefore cannot strand a worker.
pub(super) struct Scope {
    signal: watch::Sender<bool>,
    monitor: tokio::task::JoinHandle<()>,
}
impl Scope {
    pub fn new(
        mut request: watch::Receiver<bool>,
        mut owner: watch::Receiver<bool>,
        mut policy: watch::Receiver<bool>,
    ) -> Self {
        let invalid = [&request, &owner, &policy]
            .iter()
            .any(|source| *source.borrow() || source.has_changed().is_err());
        let (signal, _) = watch::channel(invalid);
        let cancelled = signal.clone();
        let monitor = tokio::spawn(async move {
            loop {
                if [&request, &owner, &policy]
                    .iter()
                    .any(|source| *source.borrow() || source.has_changed().is_err())
                {
                    cancelled.send_replace(true);
                    break;
                }
                tokio::select! {
                    _ = request.changed() => {},
                    _ = owner.changed() => {},
                    _ = policy.changed() => {},
                }
            }
        });
        Self { signal, monitor }
    }
    pub fn cancelled(&self) -> watch::Receiver<bool> {
        self.signal.subscribe()
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        self.signal.send_replace(true);
        self.monitor.abort();
    }
}
