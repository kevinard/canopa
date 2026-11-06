//! Startup discovery runs independently of D-Bus readiness. The parent owns the
//! review helper until confirmed exit, including after a bounded shutdown reply.
use super::{file_target::RecoveryStore, worker::Client};
use crate::process::DialogProcess;
use std::{sync::Arc, time::Duration};
use tokio::{io::AsyncWriteExt, sync::watch, task::JoinHandle};

pub struct RecoveryHost {
    cancel: watch::Sender<bool>,
    client: Client,
    task: Option<JoinHandle<()>>,
}
impl RecoveryHost {
    pub fn start(process: Arc<DialogProcess>, store: RecoveryStore) -> Self {
        let client = Client::new((*process).clone()).with_recovery(store.clone());
        let (cancel, cancelled) = watch::channel(false);
        let query = client.clone();
        let task = tokio::spawn(async move {
            let result = async {
                match query.recover_printer_jobs(cancelled.clone()).await {
                    Ok(Some(report)) if report.retired != 0 || report.unresolved != 0 || report.busy != 0 => {
                        eprintln!("level=info component=print_recovery event=printer_jobs retired={} unresolved={} busy={}", report.retired, report.unresolved, report.busy);
                    }
                    Ok(_) => {},
                    Err(error) => eprintln!("level=warn component=print_recovery event=printer_jobs_unconfirmed error={error}"),
                }
                if *cancelled.borrow() { return Ok(()); }
                let files = query.recovery(cancelled.clone()).await;
                let printers = query.printer_recovery(cancelled.clone()).await;
                // One unreadable catalog must not hide the other catalog's work.
                let show = match files {
                    Ok(Some(items)) => !items.is_empty(),
                    Ok(None) => false,
                    Err(error) => {
                        eprintln!("level=warn component=print_recovery event=file_list_failed error={error}");
                        true
                    }
                } | match printers {
                    Ok(Some(items)) => !items.is_empty(),
                    Ok(None) => false,
                    Err(error) => {
                        eprintln!("level=warn component=print_recovery event=printer_list_failed error={error}");
                        true
                    }
                };
                if !show || *cancelled.borrow() {
                    return Ok(());
                }
                present(&process, &store, cancelled).await
            }
            .await;
            if let Err(error) = result {
                eprintln!(
                    "level=warn component=print_recovery event=startup_review_failed error={error}"
                );
            }
        });
        Self {
            cancel,
            client,
            task: Some(task),
        }
    }
    pub async fn shutdown(mut self) {
        self.cancel.send_replace(true);
        let task = self.task.take().expect("owned recovery task");
        let (joined, pending) = tokio::join!(
            tokio::time::timeout(Duration::from_secs(15), task),
            self.client.shutdown(),
        );
        if !matches!(joined, Ok(Ok(()))) || pending != 0 {
            eprintln!("level=warn component=print_recovery event=shutdown_pending");
        }
    }
}
impl Drop for RecoveryHost {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
    }
}

async fn present(
    process: &DialogProcess,
    store: &RecoveryStore,
    mut cancelled: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    if *cancelled.borrow() {
        return Ok(());
    }
    let mut child = process.spawn("--print-recovery-dialog", None)?;
    let mut input = child.stdin.take().expect("piped recovery input");
    let data = serde_json::to_vec(store)?;
    let sending = async {
        input.write_all(&data).await?;
        input.shutdown().await
    };
    let sent = tokio::select! {
        biased;
        _ = cancelled.changed() => Ok(false),
        result = sending => result.map(|_| true),
    };
    drop(input);
    let failure = sent.as_ref().err().map(|error| error.to_string());
    if sent.unwrap_or(false) {
        tokio::select! {
            biased;
            _ = cancelled.changed() => {},
            status = child.wait() => {
                match status {
                    Ok(status) => { anyhow::ensure!(status.success(), "Recovery review stopped"); return Ok(()); }
                    Err(error) => eprintln!("level=warn component=print_recovery event=review_wait_failed error={error}"),
                }
            },
        }
    }
    if let Err(error) = child.start_kill() {
        eprintln!(
            "level=warn component=print_recovery event=review_kill_unconfirmed error={error}"
        );
    }
    if tokio::time::timeout(Duration::from_secs(2), wait_for_exit(&mut child))
        .await
        .is_err()
    {
        eprintln!("level=warn component=print_recovery event=review_retirement_pending");
        // Keep ownership; a timeout never establishes the helper's death.
        wait_for_exit(&mut child).await;
    }
    if let Some(error) = failure {
        anyhow::bail!("Recovery review input failed: {error}");
    }
    Ok(())
}

async fn wait_for_exit(child: &mut tokio::process::Child) {
    let mut reported = false;
    loop {
        match child.wait().await {
            Ok(_) => return,
            Err(error) => {
                if !reported {
                    eprintln!(
                        "level=warn component=print_recovery event=review_wait_failed error={error}"
                    );
                    reported = true;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}
