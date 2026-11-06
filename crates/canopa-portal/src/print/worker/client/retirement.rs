//! A pending exit completes the caller without releasing the owning task's
//! operation/storage/consent guards. Cleanup still waits for confirmed exit.
use super::super::{Event, Operation};
use crate::print::file_target::{Disposition, Preparation};
use tokio::sync::{oneshot, watch};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub(super) struct Pending {
    pub accepted_job: Option<i32>,
    pub file: Option<Disposition>,
}

#[derive(Clone)]
pub(super) struct Reporter {
    sender: watch::Sender<Option<Pending>>,
    pub shutdown: watch::Receiver<bool>,
    file: Option<(Preparation, bool)>,
    pub cleanup: bool,
    #[cfg(test)]
    gate: Option<std::sync::Arc<ReapGate>>,
}
impl Reporter {
    pub fn new(shutdown: watch::Receiver<bool>) -> (Self, watch::Receiver<Option<Pending>>) {
        let (sender, receiver) = watch::channel(None);
        (
            Self {
                sender,
                shutdown,
                file: None,
                cleanup: false,
                #[cfg(test)]
                gate: None,
            },
            receiver,
        )
    }
    pub fn for_file(&self, preparation: &Preparation, acknowledged: bool) -> Self {
        Self {
            file: Some((preparation.clone(), acknowledged)),
            ..self.clone()
        }
    }
    pub fn cleanup(&self) -> Self {
        Self {
            cleanup: true,
            ..self.clone()
        }
    }
    pub fn pending(&self, pid: Option<u32>, event: Option<&Event>) {
        let accepted_job = match event {
            Some(Event::Accepted { job }) => Some(*job),
            _ => None,
        };
        let file = self.file.as_ref().map(|(preparation, acknowledged)| {
            let published = *acknowledged || matches!(event, Some(Event::PublishedFile))
                || matches!(event, Some(Event::SettledFile { disposition }) if disposition.published);
            preparation.retained(published)
        });
        eprintln!("level=warn component=print event=worker_retirement_pending pid={pid:?}");
        self.sender
            .send_replace(Some(Pending { accepted_job, file }));
    }
    #[cfg(test)]
    pub fn with_gate(self, gate: Option<std::sync::Arc<ReapGate>>) -> Self {
        Self { gate, ..self }
    }
}

pub(super) async fn wait_child(
    child: &mut tokio::process::Child,
    operation: &Operation,
    reporter: &Reporter,
) -> std::process::ExitStatus {
    #[cfg(test)]
    if let Some(gate) = &reporter.gate {
        gate.wait(operation).await;
    }
    #[cfg(not(test))]
    let _ = (operation, reporter);
    let mut warned = false;
    loop {
        match child.wait().await {
            Ok(status) => return status,
            Err(error) => {
                if !warned {
                    eprintln!(
                        "level=warn component=print event=worker_exit_unconfirmed error={error:?}"
                    );
                    warned = true;
                }
                // An observation error is not proof of death. Keep the same
                // child/guards and retry; never release admission on this path.
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
}

pub(super) async fn receive<T>(
    reply: oneshot::Receiver<anyhow::Result<Option<T>>>,
    mut notices: watch::Receiver<Option<Pending>>,
    pending: impl FnOnce(Pending) -> anyhow::Result<Option<T>>,
) -> anyhow::Result<Option<T>> {
    tokio::select! {
        biased;
        result = reply => result.map_err(|_| anyhow::anyhow!("Print worker task stopped"))?,
        changed = notices.changed() => {
            changed.map_err(|_| anyhow::anyhow!("Print worker task stopped"))?;
            let notice = notices.borrow().clone().ok_or_else(|| anyhow::anyhow!("Missing worker retirement report"))?;
            pending(notice)
        }
    }
}

/// Test-only confirmation barrier. The OS child can already have exited; this
/// models retirement that cannot yet be confirmed, not an actual kernel D wait.
#[cfg(test)]
pub(crate) struct ReapGate {
    release: watch::Sender<bool>,
    filter: fn(&Operation) -> bool,
}
#[cfg(test)]
impl ReapGate {
    pub fn new(filter: fn(&Operation) -> bool) -> std::sync::Arc<Self> {
        let (release, _) = watch::channel(false);
        std::sync::Arc::new(Self { release, filter })
    }
    pub fn release(&self) {
        self.release.send_replace(true);
    }
    async fn wait(&self, operation: &Operation) {
        if !(self.filter)(operation) {
            return;
        }
        let mut released = self.release.subscribe();
        while !*released.borrow() {
            if released.changed().await.is_err() {
                break;
            }
        }
    }
}
