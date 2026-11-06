//! Optional request expiry is sender withdrawal, independent of banner display.
//! One timer and at most the registration cap of weak records are retained.
use super::{Operation, OperationState, Operations, Registry};
use std::{sync::Weak, time::Instant};
use tokio::sync::watch;

pub(super) async fn run(
    registry: Registry,
    operations: Weak<OperationState>,
    mut stopped: watch::Receiver<bool>,
) {
    loop {
        let deadline = registry
            .0
            .lock()
            .unwrap()
            .expires
            .values()
            .map(|(deadline, _)| *deadline)
            .min();
        tokio::select! {
            biased;
            _ = async { if !*stopped.borrow() { let _ = stopped.changed().await; } } => return,
            _ = registry.1.notified() => {},
            _ = tokio::time::sleep_until(deadline.unwrap_or_else(Instant::now).into()), if deadline.is_some() => {
                let due = {
                    let mut state = registry.0.lock().unwrap();
                    let ids: Vec<_> = state.expires.iter().filter(|(_, (at, _))| *at <= Instant::now()).map(|(id, _)| *id).collect();
                    ids.into_iter().filter_map(|id| state.expires.remove(&id).map(|(_, binding)| (id, binding))).collect::<Vec<_>>()
                };
                let Some(operations): Option<Operations> = operations.upgrade() else { return; };
                let Some(sender) = operations.lock().unwrap().clone() else { return; };
                for (id, binding) in due {
                    tokio::select! {
                        biased;
                        _ = async { if !*stopped.borrow() { let _ = stopped.changed().await; } } => return,
                        sent = sender.send(Operation::Expire(id, binding)) => if sent.is_err() { return; },
                    }
                }
            }
        }
    }
}
