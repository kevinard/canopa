//! Native fixture for the production Print client. Uses only owned test inputs.
use canopa_portal::{
    print::worker::{Client, Destination, JobOptions},
    process::DialogProcess,
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Seek, SeekFrom},
    os::{fd::OwnedFd, unix::net::UnixStream},
    path::PathBuf,
    time::Duration,
};
use tokio::sync::watch;

fn children() -> Vec<String> {
    let mut result = Vec::new();
    for task in std::fs::read_dir("/proc/self/task").unwrap().flatten() {
        if let Ok(text) = std::fs::read_to_string(task.path().join("children")) {
            result.extend(text.split_whitespace().map(str::to_owned));
        }
    }
    result
}
async fn wait_children(present: bool) {
    tokio::time::timeout(Duration::from_secs(15), async {
        while children().is_empty() == present {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("owned helper process state");
}
fn descriptors() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

async fn normal_snapshot(client: &Client, request: &Value) -> canopa_portal::print::worker::Frozen {
    let path = request["path"].as_str().unwrap();
    let mut source = std::fs::File::open(path).unwrap();
    let offset = request["offset"].as_u64().unwrap_or(0);
    source.seek(SeekFrom::Start(offset)).unwrap();
    let mut observed = source.try_clone().unwrap();
    let (_sender, cancelled) = watch::channel(false);
    let document = client
        .snapshot(source.into(), cancelled)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(observed.stream_position().unwrap(), offset);
    document
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let mut input = String::new();
    std::io::stdin().lock().read_line(&mut input).unwrap();
    let request: Value = serde_json::from_str(&input).unwrap();
    let executable = PathBuf::from(std::env::args_os().nth(1).unwrap());
    let client = Client::new(DialogProcess {
        executable,
        arguments: Vec::new(),
    });
    let mode = request["mode"].as_str().unwrap();
    let result = match mode {
        "snapshot" => {
            let frozen = normal_snapshot(&client, &request).await;
            json!({"snapshot": frozen.document})
        }
        "lifetime" => {
            // Warm the shared SIGCHLD machinery before comparing descriptor counts.
            drop(normal_snapshot(&client, &request).await);
            wait_children(false).await;
            tokio::time::sleep(Duration::from_millis(100)).await;
            let baseline = descriptors();
            let mut rounds = Vec::new();
            for abort in [false, true, false, true] {
                let (source, writer) = UnixStream::pair().unwrap();
                let source: OwnedFd = source.into();
                let (cancel, cancelled) = watch::channel(false);
                let owned = client.clone();
                let task = tokio::spawn(async move { owned.snapshot(source, cancelled).await });
                wait_children(true).await;
                let live = children();
                if abort {
                    task.abort();
                    assert!(matches!(task.await, Err(error) if error.is_cancelled()));
                } else {
                    cancel.send_replace(true);
                    assert!(task.await.unwrap().unwrap().is_none());
                }
                // Keep the input writer open: EOF cannot explain cancellation.
                wait_children(false).await;
                drop(writer);
                drop(cancel);
                tokio::time::sleep(Duration::from_millis(100)).await;
                let after = descriptors();
                assert_eq!(after, baseline, "retired helper descriptor count");
                rounds
                    .push(json!({"abort_future": abort, "live_helpers": live, "fds_after": after}));
            }
            json!({"baseline_fds": baseline, "rounds": rounds})
        }
        "limits" => {
            let first = normal_snapshot(&client, &request).await;
            let second = normal_snapshot(&client, &request).await;
            let source = std::fs::File::open(request["path"].as_str().unwrap()).unwrap();
            let (_sender, cancelled) = watch::channel(false);
            assert!(client.snapshot(source.into(), cancelled).await.is_err());
            drop(first);
            let replacement = normal_snapshot(&client, &request).await;
            assert_eq!(second.document.bytes, replacement.document.bytes);
            json!({"two_retained_documents": true, "third_refused": true, "released_slot_reused": true})
        }
        "submit" | "cancel_submit" | "reject_submit" => {
            let mut interrupt =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::user_defined1())
                    .unwrap();
            let frozen = normal_snapshot(&client, &request).await;
            if request["mutate_after_snapshot"].as_bool().unwrap_or(false) {
                std::fs::write(request["path"].as_str().unwrap(), b"changed after snapshot")
                    .unwrap();
            }
            let destination: Destination =
                serde_json::from_value(request["destination"].clone()).unwrap();
            let options: JobOptions = serde_json::from_value(request["options"].clone()).unwrap();
            let (cancel, cancelled) = watch::channel(false);
            let submission = client.submit(
                &frozen,
                destination,
                "Canopa private Rust print worker".into(),
                options,
                cancelled,
            );
            tokio::pin!(submission);
            let submitted = tokio::select! {
                value = &mut submission => value,
                _ = interrupt.recv() => { cancel.send_replace(true); submission.await },
            };
            if mode == "reject_submit" {
                assert!(submitted.is_err());
                json!({"submission_refused": true, "snapshot": frozen.document})
            } else {
                let submitted = submitted.unwrap();
                if mode == "cancel_submit" {
                    assert!(submitted.is_none());
                } else {
                    assert!(submitted.is_some());
                }
                json!({"accepted_job": submitted, "snapshot": frozen.document})
            }
        }
        _ => panic!("unknown fixture mode"),
    };
    wait_children(false).await;
    println!(
        "{}",
        json!({"status":"passed", "mode": mode, "result": result, "children_terminal": true})
    );
}
