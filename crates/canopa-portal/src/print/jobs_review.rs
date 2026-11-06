//! Backend-owned live-job review. Its child holds display data, never job proof.
use super::{
    Context,
    authentication::Broker,
    live_jobs::{Job, Status},
    worker::{Client, Destination, Printer},
};
use crate::process::DialogProcess;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    os::{
        fd::{AsFd, AsRawFd},
        unix::net::UnixStream,
    },
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::{OwnedSemaphorePermit, oneshot, watch},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub context: Context,
    pub channel: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Command {
    List { destination: Option<Destination> },
    Authenticate { key: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Entry {
    pub key: String,
    pub job: i32,
    pub title: String,
    pub status: Status,
    pub authenticate: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Response {
    Listing {
        printers: Vec<Printer>,
        destination: Option<Destination>,
        entries: Vec<Entry>,
    },
    SignedIn {
        acknowledged: bool,
        cancelled: bool,
    },
    Failed {
        message: String,
    },
}
const COMMAND_BYTES: usize = 4096;

async fn serve(
    channel: tokio::net::UnixStream,
    client: &Client,
    process: &DialogProcess,
    context: &Context,
    cancelled: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let (read, mut write) = channel.into_split();
    let mut read = BufReader::new(read);
    let mut jobs: HashMap<String, Job> = HashMap::new();
    loop {
        let mut bytes = Vec::new();
        let count = (&mut read)
            .take(COMMAND_BYTES as u64 + 1)
            .read_until(b'\n', &mut bytes)
            .await?;
        if count == 0 {
            return Ok(());
        }
        anyhow::ensure!(
            bytes.len() <= COMMAND_BYTES && bytes.last() == Some(&b'\n'),
            "Invalid printer review command"
        );
        let command: Command = serde_json::from_slice(&bytes)?;
        let broker = Broker::new(process.clone(), context.clone());
        let action = client.clone().with_authentication(Some(broker));
        let response = match command {
            Command::List { destination } => {
                jobs.clear();
                let listing = async {
                    let printers = action
                        .printers(cancelled.clone())
                        .await?
                        .ok_or_else(|| anyhow::anyhow!("Printer listing cancelled"))?;
                    let selected = if let Some(destination) = destination {
                        printers
                            .iter()
                            .any(|printer| printer.destination == destination)
                            .then_some(destination)
                    } else {
                        printers
                            .iter()
                            .find(|printer| printer.is_default)
                            .or(printers.first())
                            .map(|printer| printer.destination.clone())
                    };
                    let listed = if let Some(destination) = &selected {
                        action
                            .live_jobs(destination.clone(), cancelled.clone())
                            .await?
                            .ok_or_else(|| anyhow::anyhow!("Printer job listing cancelled"))?
                    } else {
                        vec![]
                    };
                    let mut entries = Vec::new();
                    for job in listed {
                        let key = job.key()?;
                        entries.push(Entry {
                            key: key.clone(),
                            job: job.job,
                            title: job.title.clone(),
                            status: job.status,
                            authenticate: job.can_authenticate(),
                        });
                        anyhow::ensure!(
                            jobs.insert(key, job).is_none(),
                            "Duplicate printer selection"
                        );
                    }
                    Ok::<_, anyhow::Error>(Response::Listing {
                        printers,
                        destination: selected,
                        entries,
                    })
                }
                .await;
                listing.unwrap_or_else(|error| {
                    jobs.clear();
                    eprintln!(
                        "level=warn component=print_jobs event=list_unavailable error={error}"
                    );
                    Response::Failed {
                        message: "Couldn’t check printer jobs. Refresh to try again.".into(),
                    }
                })
            }
            Command::Authenticate { key } => {
                anyhow::ensure!(
                    key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit()),
                    "Invalid printer selection"
                );
                let job = jobs
                    .remove(&key)
                    .filter(Job::can_authenticate)
                    .ok_or_else(|| anyhow::anyhow!("Stale printer selection; refresh required"))?;
                // One selection authorizes one explicit action. Any subsequent
                // attempt requires fresh listing and another user activation.
                jobs.clear();
                match action.authenticate_held_job(job, cancelled.clone()).await {
                    Ok(Some(())) => Response::SignedIn {
                        acknowledged: true,
                        cancelled: false,
                    },
                    Ok(None) => Response::SignedIn {
                        acknowledged: false,
                        cancelled: true,
                    },
                    Err(error) => {
                        eprintln!(
                            "level=warn component=print_jobs event=sign_in_unconfirmed error={error}"
                        );
                        Response::SignedIn {
                            acknowledged: false,
                            cancelled: false,
                        }
                    }
                }
            }
        };
        let mut bytes = serde_json::to_vec(&response)?;
        anyhow::ensure!(
            bytes.len() <= crate::process::MAX_MESSAGE_BYTES,
            "Printer review response exceeds limits"
        );
        bytes.push(b'\n');
        tokio::time::timeout(Duration::from_secs(2), write.write_all(&bytes)).await??;
    }
}

pub(super) async fn present(
    process: Arc<DialogProcess>,
    context: Context,
    cancelled: watch::Receiver<bool>,
    permit: OwnedSemaphorePermit,
) -> anyhow::Result<()> {
    let (alive, lifetime) = oneshot::channel();
    let (reply, result) = oneshot::channel();
    tokio::spawn(async move {
        // The review and its own bounded helpers remain owned after the D-Bus
        // method future is dropped. Admission lasts through confirmed retirement.
        let _permit = permit;
        let client = Client::new((*process).clone());
        let result = run(&process, context, cancelled, lifetime, &client).await;
        client.shutdown().await;
        while client.idle().await.is_err() {
            eprintln!("level=warn component=print_jobs event=worker_retirement_pending");
        }
        let _ = reply.send(result);
    });
    let result = result
        .await
        .map_err(|_| anyhow::anyhow!("Printer review stopped"))?;
    drop(alive);
    result
}
async fn run(
    process: &DialogProcess,
    context: Context,
    mut cancelled: watch::Receiver<bool>,
    lifetime: oneshot::Receiver<()>,
    client: &Client,
) -> anyhow::Result<()> {
    if *cancelled.borrow() || cancelled.has_changed().is_err() {
        return Ok(());
    }
    let (parent, child) = UnixStream::pair()?;
    parent.set_nonblocking(true)?;
    let channel = tokio::net::UnixStream::from_std(parent)?;
    let configuration = Configuration {
        context: context.clone(),
        channel: child.as_raw_fd(),
    };
    let mut helper = process.spawn_inheriting("--print-jobs-dialog", None, &[child.as_fd()])?;
    drop(child);
    let mut input = helper.stdin.take().expect("piped printer review input");
    let output = helper.stdout.take().expect("piped printer review output");
    let (stop, stopped) = watch::channel(false);
    let interaction = async {
        input
            .write_all(&serde_json::to_vec(&configuration)?)
            .await?;
        input.shutdown().await?;
        drop(input);
        serve(channel, client, process, &context, stopped).await?;
        let mut result = Vec::new();
        output.take(1025).read_to_end(&mut result).await?;
        anyhow::ensure!(
            result.len() <= 1024 && serde_json::from_slice::<bool>(&result)?,
            "Printer review response unavailable"
        );
        Ok::<_, anyhow::Error>(())
    };
    let (result, withdrawn, early_status) = tokio::select! {
        biased;
        _ = async { if !*cancelled.borrow() { let _ = cancelled.changed().await; } } => (Ok(()), true, None),
        _ = lifetime => (Ok(()), true, None),
        status = helper.wait() => match status {
            Ok(status) => {
                let result = if status.success() { Ok(()) } else { Err(anyhow::anyhow!("Printer review stopped")) };
                (result, true, Some(status))
            }
            Err(error) => (Err(error.into()), true, None),
        },
        result = interaction => (result, false, None),
    };
    stop.send_replace(true);
    let mut status = if early_status.is_some() {
        early_status
    } else if result.is_ok() && !withdrawn {
        tokio::time::timeout(Duration::from_secs(2), helper.wait())
            .await
            .ok()
            .and_then(Result::ok)
    } else {
        None
    };
    if status.is_none() {
        let _ = helper.start_kill();
    }
    let mut warned = false;
    while status.is_none() {
        match tokio::time::timeout(Duration::from_secs(2), helper.wait()).await {
            Ok(Ok(confirmed)) => status = Some(confirmed),
            _ => {
                if !warned {
                    eprintln!("level=warn component=print_jobs event=review_retirement_pending");
                    warned = true;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
    if !withdrawn && result.is_ok() {
        anyhow::ensure!(
            status.expect("confirmed helper exit").success(),
            "Printer review stopped"
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};

    #[tokio::test]
    async fn review_child_exit_cancels_an_inflight_worker_before_its_deadline() {
        // A real child exits while its private RPC awaits another real child.
        // The latter models blocked metadata, not an actual CUPS transaction.
        let folder = tempfile::tempdir().unwrap();
        let wrapper = folder.path().join("review.py");
        let started = folder.path().join("worker-started");
        let exited = folder.path().join("worker-exited");
        fs::write(
            &wrapper,
            format!(
                r#"#!/usr/bin/python3
from pathlib import Path
import json,os,socket,sys,time
request=json.load(sys.stdin)
if '--print-jobs-dialog' in sys.argv:
    channel=socket.socket(fileno=request['channel'])
    channel.sendall(b'{{"command":"list","destination":null}}\n')
    deadline=time.monotonic()+5
    while not Path({started:?}).exists():
        assert time.monotonic()<deadline
        time.sleep(.005)
    print('true',flush=True)
else:
    assert '--print-worker' in sys.argv and request['operation']['operation']=='printers'
    Path({started:?}).write_text(str(os.getpid()))
    os.read(request['cancel'],1)
    Path({exited:?}).write_text('cancelled')
    print('{{"event":"printers","printers":[]}}',flush=True)
"#,
                started = started.to_str().unwrap(),
                exited = exited.to_str().unwrap()
            ),
        )
        .unwrap();
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
        let process = Arc::new(DialogProcess {
            executable: wrapper,
            arguments: vec![],
        });
        let admission = Arc::new(tokio::sync::Semaphore::new(1));
        let (_cancel, cancelled) = watch::channel(false);
        tokio::time::timeout(
            Duration::from_secs(8),
            present(
                process,
                Context {
                    app_id: "org.canopa.Portal".into(),
                    parent_window: String::new(),
                    title: "Printer jobs".into(),
                    modal: false,
                },
                cancelled,
                admission.clone().acquire_owned().await.unwrap(),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(fs::read_to_string(exited).unwrap(), "cancelled");
        assert_eq!(
            admission.available_permits(),
            1,
            "review admission outlived confirmed retirement"
        );
    }
}
