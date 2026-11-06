use crate::support;
use canopa_notify::{Daemon, DaemonOptions, NotificationInput};
use canopa_notify_client::{MAX_SUMMARY_BYTES, SUMMARY_REQUEST, SummarySubscription};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::Child;
use std::time::Duration;

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

#[tokio::test]
async fn standard_session_socket_streams_bounded_state_and_recovers_after_restart() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("canopa-notify/control.sock");
    let database = temp.path().join("history.db");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    for _ in 0..20 {
        seed.receive_notification(NotificationInput {
            app_name: "Summary fixture".into(),
            summary: "Private content".into(),
            body: "x".repeat(60_000),
            ..Default::default()
        })
        .await
        .unwrap();
    }
    seed.shutdown().await.unwrap();
    drop(seed);
    let command = || {
        let mut command = support::canopa_notify_command();
        command
            .env("XDG_RUNTIME_DIR", temp.path())
            .env("XDG_CONFIG_HOME", temp.path().join("config"));
        command
    };
    let launch = || {
        Process(
            command()
                .args([
                    "daemon",
                    "--headless",
                    "--database",
                    support::path_arg(&database),
                ])
                .spawn()
                .unwrap(),
        )
    };
    let mut process = launch();
    support::wait_for_status_with_child(&socket, &mut process.0);
    let (mut first, initial) = SummarySubscription::connect(&socket).await.unwrap();
    assert_eq!(initial.requires_attention, 20);
    assert!(!initial.do_not_disturb);
    let mut readers = Vec::new();
    for _ in 0..7 {
        readers.push(SummarySubscription::connect(&socket).await.unwrap().0);
    }
    let mut refused = UnixStream::connect(&socket).unwrap();
    refused
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    refused.write_all(SUMMARY_REQUEST.as_bytes()).unwrap();
    let mut line = String::new();
    BufReader::new(refused).read_line(&mut line).unwrap();
    assert!(line.starts_with("error too many summary"));
    assert!(line.len() <= MAX_SUMMARY_BYTES);
    assert!(
        command()
            .args(["ctl", "dnd", "toggle"])
            .status()
            .unwrap()
            .success()
    );
    let changed = tokio::time::timeout(Duration::from_secs(2), first.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(changed.do_not_disturb);
    assert_eq!(changed.requires_attention, 20);
    process.0.kill().unwrap();
    process.0.wait().unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), first.next())
            .await
            .unwrap()
            .unwrap(),
        None
    );
    process = launch();
    support::wait_for_status_with_child(&socket, &mut process.0);
    let (_, restarted) = SummarySubscription::connect(&socket).await.unwrap();
    assert_ne!(restarted.instance, initial.instance);
    assert_eq!(restarted.requires_attention, 20);
    assert!(restarted.do_not_disturb);
}
