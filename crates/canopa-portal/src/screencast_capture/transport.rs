use super::*;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub(super) async fn start(
    process: &DialogProcess,
    input: Input,
    mut cancelled: watch::Receiver<bool>,
    permit: (
        tokio::sync::OwnedSemaphorePermit,
        tokio::sync::OwnedSemaphorePermit,
    ),
) -> anyhow::Result<Recording> {
    anyhow::ensure!(!*cancelled.borrow(), "Capture cancelled");
    let mut bytes = serde_json::to_vec(&input)?;
    anyhow::ensure!(bytes.len() < FRAME_BYTES, "Capture request too large");
    bytes.push(b'\n');
    // Unlike UI helpers this worker must survive backend death to send Stop.
    let mut child = tokio::process::Command::new(&process.executable)
        .args(&process.arguments)
        .arg("--screencast-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .env_remove("XDG_ACTIVATION_TOKEN")
        .env_remove("DESKTOP_STARTUP_ID")
        .kill_on_drop(false)
        .spawn()?;
    let mut input_pipe = child.stdin.take().expect("piped input");
    let output = child.stdout.take().expect("piped output");
    let (lease, mut released) = oneshot::channel();
    let (ready, response) = oneshot::channel();
    let (done, finished) = oneshot::channel();
    let (closed, stopped) = watch::channel(false);
    let owner = input.compositor.clone();
    tokio::spawn(async move {
        let retained = permit;
        let interaction = async {
            input_pipe.write_all(&bytes).await?;
            input_pipe.flush().await?;
            let mut line = Vec::new();
            BufReader::new(output.take(RESPONSE_BYTES as u64 + 1))
                .read_until(b'\n', &mut line)
                .await?;
            anyhow::ensure!(
                line.len() <= RESPONSE_BYTES && line.last() == Some(&b'\n'),
                "Invalid capture worker response"
            );
            let streams: Vec<Stream> = serde_json::from_slice(&line)?;
            anyhow::ensure!(
                streams.len() == input.sources.len()
                    && streams.iter().zip(&input.sources).all(|(stream, source)| {
                        stream.node != 0
                            && stream.node != u32::MAX
                            && &stream.source == source
                            && stream.mapping.as_ref().is_none_or(|mapping| {
                                mapping.valid()
                                    && mapping.matches(source)
                                    && stream
                                        .geometry
                                        .is_none_or(|geometry| geometry.size == mapping.size)
                            })
                            && match (&stream.source, stream.geometry) {
                                (Source::Monitor(_), Some(geometry)) => geometry.valid(),
                                (Source::Window(_), None) => true,
                                _ => false,
                            }
                    })
                    && streams.iter().map(|v| v.node).collect::<HashSet<_>>().len()
                        == streams.len(),
                "Invalid capture streams"
            );
            Ok::<_, anyhow::Error>(streams)
        };
        let result = tokio::select! {
            biased;
            _ = &mut released => Err(anyhow::anyhow!("Capture cancelled")),
            _ = async { if !*cancelled.borrow() { let _ = cancelled.changed().await; } } => Err(anyhow::anyhow!("Capture cancelled")),
            result = tokio::time::timeout(Duration::from_secs(55), interaction) => result.map_err(anyhow::Error::from).and_then(|v| v),
        };
        // A completed Start request's watch sender can disappear normally.
        drop(cancelled);
        if result.is_ok() {
            let delivered = ready.send(result).is_ok();
            if delivered {
                tokio::select! {
                    _ = &mut released => {},
                    _ = child.wait() => {},
                }
            }
        } else {
            let _ = ready.send(result);
        }
        drop(input_pipe);
        // Create can finish after cancellation; the worker retains the reply
        // and cleans its returned session before exit. Allow that cleanup time.
        let result = match tokio::time::timeout(Duration::from_secs(60), child.wait()).await {
            Ok(Ok(status)) if status.success() => Ok(()),
            Ok(Ok(_)) | Ok(Err(_)) => Err(anyhow::anyhow!("Capture cleanup unconfirmed")),
            Err(_) => {
                let _ = child.kill().await;
                Err(anyhow::anyhow!("Capture worker did not retire"))
            }
        };
        drop(retained);
        closed.send_replace(true);
        let _ = done.send(result);
    });
    let streams = response.await??;
    let bindings = bindings::Bindings::from_streams(&owner, &streams)?;
    Ok(Recording {
        streams,
        bindings,
        lease: Some(lease),
        closed: stopped,
        done: finished,
    })
}
