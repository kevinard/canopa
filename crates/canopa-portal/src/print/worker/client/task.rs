use super::super::{Event, Input, Operation};
use super::authentication;
use super::retirement::{Reporter, wait_child};
use std::{
    os::fd::{AsFd, AsRawFd, OwnedFd},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::{oneshot, watch},
};

const FRAME_BYTES: usize = crate::process::MAX_MESSAGE_BYTES;
const DEADLINE: Duration = Duration::from_secs(60);
const INTERACTIVE_DEADLINE: Duration = Duration::from_secs(300);
const STOP_GRACE: Duration = Duration::from_secs(10);
const REAP_GRACE: Duration = Duration::from_secs(2);

pub(super) struct Execution {
    pub result: anyhow::Result<Option<Event>>,
    pub created: Option<i32>,
    pub accepted: bool,
    pub prepared: Option<crate::print::file_target::PublicationInfo>,
}

async fn read_event(reader: &mut BufReader<tokio::process::ChildStdout>) -> anyhow::Result<Event> {
    let mut bytes = Vec::new();
    (&mut *reader)
        .take(FRAME_BYTES as u64 + 1)
        .read_until(b'\n', &mut bytes)
        .await?;
    anyhow::ensure!(
        bytes.len() <= FRAME_BYTES && bytes.last() == Some(&b'\n'),
        "Invalid print worker frame"
    );
    Ok(serde_json::from_slice(&bytes)?)
}

pub(super) async fn run(
    client: &super::Client,
    operation: Operation,
    mut descriptors: Vec<OwnedFd>,
    mut cancelled: watch::Receiver<bool>,
    lifetime: &mut oneshot::Receiver<()>,
    reporter: &Reporter,
    broker: Option<std::sync::Arc<crate::print::authentication::Broker>>,
) -> Execution {
    let process = &client.process;
    let runtime = &client.runtime;
    let mut execution = Execution {
        result: Ok(None),
        created: None,
        accepted: false,
        prepared: None,
    };
    if *cancelled.borrow()
        || cancelled.has_changed().is_err()
        || (!reporter.cleanup && *reporter.shutdown.borrow())
    {
        return execution;
    }
    let result = async {
        let (cancel_read, cancel_write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;
        let cancel_read = rustix::io::fcntl_dupfd_cloexec(&cancel_read, 3)?;
        let broker = broker.filter(|_| operation.uses_cups());
        let auth = if let Some(broker) = broker {
            let (parent, child) = std::os::unix::net::UnixStream::pair()?;
            parent.set_nonblocking(true)?;
            let child = rustix::io::fcntl_dupfd_cloexec(&child, 3)?;
            Some((tokio::net::UnixStream::from_std(parent)?, child, broker))
        } else { None };
        let authentication = auth.as_ref().map(|(_,child,_)| child.as_raw_fd());
        let cups_runtime = if operation.uses_cups() { runtime.inherit(&mut descriptors)? } else { Default::default() };
        let input = Input { cancel: cancel_read.as_raw_fd(), authentication, cups_runtime, operation: operation.clone() };
        let deadline = if auth.is_some() { INTERACTIVE_DEADLINE } else { DEADLINE };
        let auth = auth.map(|(channel, child, broker)| { descriptors.push(child); (channel, broker) });
        let (auth_cancel, auth_cancelled) = watch::channel(false);
        descriptors.push(cancel_read);
        let inherited = descriptors.iter().map(AsFd::as_fd).collect::<Vec<_>>();
        let mut child = process.spawn_inheriting("--print-worker", None, &inherited)?;
        drop(descriptors);
        let mut input_pipe = child.stdin.take().expect("piped stdin");
        let mut output = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut input_bytes = serde_json::to_vec(&input)?;
        input_bytes.push(b'\n');
        let mut cancel_write = Some(cancel_write);
        let mut stopped = false;
        let mut timed_out = false;
        let mut shutdown = reporter.shutdown.clone();
        let response = {
            let events = async {
                input_pipe.write_all(&input_bytes).await?;
                input_pipe.shutdown().await?;
                drop(input_pipe);
                for _ in 0..3 {
                    let event = read_event(&mut output).await?;
                    match &event {
                        Event::Created { job } => {
                            anyhow::ensure!(*job > 0 && execution.created.is_none() && matches!(operation, Operation::Submit { .. }), "Invalid print job receipt");
                            execution.created = Some(*job);
                        }
                        Event::Accepted { job } => {
                            anyhow::ensure!(Some(*job) == execution.created, "Print acceptance has no owned job");
                            execution.accepted = true;
                            return Ok::<_, anyhow::Error>(event);
                        }
                        Event::PublishedFile if matches!(operation, Operation::PublishFile { .. } | Operation::PublishRecoveredFile { .. }) => {
                            execution.accepted = true;
                            return Ok(event);
                        }
                        Event::Failed { message } => anyhow::bail!("{message}"),
                        Event::AuthenticationCancelled if operation.uses_cups() => return Ok(event),
                        Event::LiveJobs { .. } if matches!(operation, Operation::ListLiveJobs { .. }) => return Ok(event),
                        Event::AuthenticatedJob { job } if matches!(&operation, Operation::AuthenticateHeldJob { job: expected } if *job == expected.job) => return Ok(event),
                        Event::RetiredPrinterJobs { report } if matches!(operation, Operation::RecoverPrinterJobs { .. }) => { report.validate()?; return Ok(event); },
                        Event::PrinterRecovery { .. } if matches!(operation, Operation::ListPrinterRecovery { .. }) => return Ok(event),
                        Event::DismissedPrinterRecovery { id } if matches!(&operation, Operation::DismissPrinterRecovery { item, .. } if *id == item.id) => return Ok(event),
                        Event::Snapshot { .. } if matches!(operation, Operation::Snapshot { .. }) => return Ok(event),
                        Event::Rendered { .. } if matches!(operation, Operation::Render { .. }) => return Ok(event),
                        Event::Printers { .. } if matches!(operation, Operation::Printers) => return Ok(event),
                        Event::Recovery { .. } if matches!(operation, Operation::ListRecovery { .. }) => return Ok(event),
                        Event::Recovered { .. } if matches!(operation, Operation::SnapshotRecovery { .. }) => return Ok(event),
                        Event::DismissedRecovery { id } if matches!(&operation, Operation::DismissRecovery { id: expected, .. } if id == expected) => return Ok(event),
                        Event::CustomMedia { .. } if matches!(operation, Operation::CustomMedia { .. }) => return Ok(event),
                        Event::CheckedPrinter { .. } if matches!(operation, Operation::CheckPrinter { .. }) => return Ok(event),
                        Event::Capabilities { .. } if matches!(operation, Operation::Capabilities { .. }) => return Ok(event),
                        Event::RetiredJob { job } if matches!(operation, Operation::CancelJob { job: expected, .. } if *job == expected) => return Ok(event),
                        Event::SettledFile { .. } if matches!(operation, Operation::SettleFile { .. }) => return Ok(event),
                        Event::VerifiedFile if matches!(operation, Operation::VerifyFile { .. }) => return Ok(event),
                        Event::PreparedFile { publication } if matches!(operation, Operation::PrepareFile { .. }) => {
                            execution.prepared = Some(publication.clone());
                            return Ok(event);
                        }
                        _ => anyhow::bail!("Unexpected print worker event"),
                    }
                }
                anyhow::bail!("Too many print worker events")
            };
            let interaction = async {
                tokio::pin!(events);
                if let Some((channel, broker)) = auth {
                    tokio::select! {
                        response = &mut events => response,
                        result = authentication::serve(channel, broker, &operation, auth_cancelled, !reporter.cleanup && !matches!(operation, Operation::RecoverPrinterJobs { interactive: false, .. }), runtime.has_image()) => {
                            result?;
                            events.await
                        }
                    }
                } else { events.await }
            };
            tokio::pin!(interaction);
            let stop = async {
                tokio::select! {
                    _ = async { if !*cancelled.borrow() { let _ = cancelled.changed().await; } } => {},
                    _ = &mut *lifetime => {},
                    _ = async {
                        if reporter.cleanup { std::future::pending::<()>().await; }
                        if !*shutdown.borrow() { let _ = shutdown.changed().await; }
                    } => {},
                }
            };
            tokio::pin!(stop);
            tokio::select! {
                biased;
                _ = &mut stop => {
                    stopped = true;
                    auth_cancel.send_replace(true);
                    drop(cancel_write.take());
                    tokio::time::timeout(STOP_GRACE, &mut interaction).await.unwrap_or_else(|_| Err(anyhow::anyhow!("Print worker did not stop")))
                }
                response = &mut interaction => response,
                _ = tokio::time::sleep(deadline) => {
                    timed_out = true;
                    auth_cancel.send_replace(true);
                    drop(cancel_write.take());
                    tokio::time::timeout(STOP_GRACE, &mut interaction).await.unwrap_or_else(|_| Err(anyhow::anyhow!("Print worker deadline exceeded")))
                }
            }
        };
        let exited = if response.is_ok() {
            tokio::time::timeout(REAP_GRACE, wait_child(&mut child, &operation, reporter)).await.ok()
        } else { None };
        let status = match exited {
            Some(status) => status,
            None => {
                let _ = child.start_kill();
                match tokio::time::timeout(REAP_GRACE, wait_child(&mut child, &operation, reporter)).await {
                    Ok(status) => status,
                    Err(_) => {
                        reporter.pending(child.id(), response.as_ref().ok());
                        // The owning task keeps every guard until actual exit.
                        // Its caller is independently released by this notice.
                        wait_child(&mut child, &operation, reporter).await
                    }
                }
            }
        };
        if execution.accepted { return response.map(Some); }
        if matches!(response,Ok(Event::AuthenticationCancelled)) { return Ok(None); }
        if stopped { return Ok(None); }
        anyhow::ensure!(!timed_out, "Print worker deadline exceeded");
        anyhow::ensure!(status.success(), "Print worker stopped ({status}): {}",
            response.as_ref().err().map(|error| format!("{error:#}")).unwrap_or_else(|| "no protocol error was reported".into()));
        response.map(Some)
    }.await;
    execution.result = result;
    execution
}
