//! Retained real Client/Broker parent; its child protocol is fixture-controlled.
use canopa_portal::{
    print::file_target::RecoveryStore, print::worker::Client, process::DialogProcess,
};
use serde_json::{Value, json};
use std::{io::BufRead, path::PathBuf, time::Instant};
use tokio::sync::watch;

fn children() -> Vec<u32> {
    let mut result = Vec::new();
    for task in std::fs::read_dir("/proc/self/task").unwrap().flatten() {
        if let Ok(text) = std::fs::read_to_string(task.path().join("children")) {
            result.extend(text.split_whitespace().map(|s| s.parse::<u32>().unwrap()));
        }
    }
    result.sort_unstable();
    result.dedup();
    result
}

fn fds() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}

fn image_descriptors() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .flatten()
        .filter(|entry| {
            std::fs::read_link(entry.path())
                .is_ok_and(|path| path.to_string_lossy().contains("memfd:canopa-print-client"))
        })
        .count()
}

fn emit(value: Value) {
    println!("{value}");
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let mut args = std::env::args_os().skip(1);
    let executable = PathBuf::from(args.next().unwrap());
    let folder = PathBuf::from(args.next().unwrap());
    assert!(args.next().is_none());
    let client = Client::new(DialogProcess {
        executable,
        arguments: vec![folder.clone().into_os_string()],
    })
    .with_recovery(RecoveryStore::new(&folder.join("recovery")).unwrap());
    emit(
        json!({"event":"ready", "pid":std::process::id(), "image_descriptors":image_descriptors()}),
    );
    let mut baseline = None;
    for line in std::io::stdin().lock().lines() {
        let request: Value = serde_json::from_str(&line.unwrap()).unwrap();
        if request["action"] == "stop" {
            assert_eq!(client.shutdown().await, 0);
            emit(json!({"event":"stopped", "children":children(), "fds":fds()}));
            return;
        }
        let id = request["id"].as_str().unwrap();
        let authenticated = request["authenticated"].as_bool().unwrap();
        let interruption = request["interruption"].as_str().unwrap_or("none");
        assert!(["none", "cancel", "drop"].contains(&interruption));
        let started = Instant::now();
        let (cancel, cancelled) = watch::channel(false);
        let owned = client.clone();
        let mut work = tokio::spawn(async move {
            if authenticated {
                owned
                    .review_printer_jobs(cancelled)
                    .await
                    .map(|v| v.map(|_| ()))
            } else {
                owned.printers(cancelled).await.map(|v| v.map(|_| ()))
            }
            .map_err(|error| format!("{error:#}"))
        });
        let mut interrupted = false;
        let response = if interruption == "none" {
            work.await
        } else {
            let marker = folder.join(format!("{id}.prompt-ready"));
            let pending = async {
                tokio::time::timeout(std::time::Duration::from_secs(10), async {
                    while !marker.exists() {
                        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    }
                })
                .await
                .expect("credential helper entered before interruption");
            };
            tokio::select! {
                value = &mut work => value,
                _ = pending => {
                    interrupted = true;
                    if interruption == "drop" {
                        work.abort();
                    } else {
                        cancel.send_replace(true);
                    }
                    work.await
                }
            }
        };
        client.idle().await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while !children().is_empty() {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("all direct children reaped");
        drop(cancel);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let (kind, error) = match response {
            Ok(Ok(Some(()))) => ("success", None),
            Ok(Ok(None)) => ("cancelled", None),
            Ok(Err(message)) => ("error", Some(message)),
            Err(error) if error.is_cancelled() => ("dropped", None),
            Err(error) => panic!("operation task: {error}"),
        };
        let descriptors = fds();
        let expected = *baseline.get_or_insert(descriptors);
        assert_eq!(
            descriptors, expected,
            "descriptor count after retirement: {id}"
        );
        assert_eq!(image_descriptors(), 1, "one configured retained image");
        let maps = std::fs::read_to_string("/proc/self/maps").unwrap();
        assert!(!maps.contains("libcups.so") && !maps.contains("memfd:canopa-print-client"));
        emit(
            json!({"event":"result", "id":id, "authenticated":authenticated,
            "interrupted":interrupted,"kind":kind,"error":error,"seconds":started.elapsed().as_secs_f64(),
            "fds":descriptors,"baseline_fds":expected,"image_descriptors":image_descriptors(),
            "children":children(),"parent_cups_mapped":false}),
        );
    }
    assert_eq!(client.shutdown().await, 0);
}
