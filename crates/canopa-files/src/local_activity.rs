//! Shared admission for local work and safe device removal. This grants no
//! filesystem or device authority; each workflow still validates its own intent.
use futures_channel::mpsc;
use std::{
    io,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

#[derive(Default)]
struct State {
    work: usize,
    removing: bool,
}

struct Gate {
    state: Mutex<State>,
    changed: Condvar,
    notice: Mutex<mpsc::Sender<()>>,
    notices: Mutex<Option<mpsc::Receiver<()>>>,
}

impl Default for Gate {
    fn default() -> Self {
        let (notice, notices) = mpsc::channel(0);
        Self {
            state: Mutex::default(),
            changed: Condvar::new(),
            notice: Mutex::new(notice),
            notices: Mutex::new(Some(notices)),
        }
    }
}

impl Gate {
    fn publish(&self) {
        let _ = self
            .notice
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .try_send(());
    }
}

/// One owning application's filesystem and custom-action admission.
#[derive(Clone, Default)]
pub struct LocalActivity(Arc<Gate>);

/// Retained until the exact platform removal request has actually settled.
/// Dropping a window must not release it. It does not authorize a GIO request.
pub struct RemovalGuard(Arc<Gate>);

struct Accepted(Arc<Gate>);

/// Clones share one accepted-work hold, including worker teardown and saving.
#[derive(Clone)]
pub(crate) struct Work(Arc<Accepted>);

impl LocalActivity {
    /// Atomically excludes new execution while no accepted work exists. Work
    /// submitted afterward is retained, but waits before filesystem preparation.
    pub fn try_remove(&self) -> io::Result<RemovalGuard> {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.removing {
            return Err(io::Error::other("A device removal is already in progress."));
        }
        if state.work != 0 {
            return Err(io::Error::other(
                "Wait for Files operations and custom actions to finish, or cancel them, before removing a device.",
            ));
        }
        state.removing = true;
        drop(state);
        self.0.publish();
        Ok(RemovalGuard(self.0.clone()))
    }

    /// Includes submissions awaiting admission, accepted work and device removal.
    pub fn holds_application(&self) -> bool {
        let state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        state.removing || state.work != 0
    }

    pub(crate) fn work(&self) -> Work {
        self.0.state.lock().unwrap_or_else(|e| e.into_inner()).work += 1;
        self.0.publish();
        Work(Arc::new(Accepted(self.0.clone())))
    }

    /// One coalesced host wakeup, including the final worker's actual retirement.
    pub fn take_notices(&self) -> Option<mpsc::Receiver<()>> {
        self.0
            .notices
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }
}

impl Work {
    /// Once ready, this accepted hold itself prevents another removal starting.
    pub(crate) fn ready(&self) -> bool {
        !self
            .0
            .0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .removing
    }
    /// Queue/command workers may wait here; frontend and admission callers never
    /// block. Cancellation and owner teardown remain responsive while waiting.
    pub(crate) fn wait(&self, cancelled: impl Fn() -> bool) -> io::Result<()> {
        let gate = &self.0.0;
        let mut state = gate.state.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if cancelled() {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Work cancelled before device removal finished.",
                ));
            }
            if !state.removing {
                return Ok(());
            }
            state = gate
                .changed
                .wait_timeout(state, Duration::from_millis(100))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}

impl Drop for Accepted {
    fn drop(&mut self) {
        self.0.state.lock().unwrap_or_else(|e| e.into_inner()).work -= 1;
        self.0.publish();
    }
}

impl Drop for RemovalGuard {
    fn drop(&mut self) {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .removing = false;
        self.0.changed.notify_all();
        self.0.publish();
    }
}
