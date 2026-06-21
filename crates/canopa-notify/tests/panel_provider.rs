mod support;

use assert_cmd::Command;
use canopa_notify::{Daemon, DaemonOptions, NotificationInput};
use canopa_panel_protocol::ProviderResponse;
use predicates::str::contains;
use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[test]
fn panel_provider_returns_provider_visible_error_when_daemon_is_unavailable() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("missing.sock");

    Command::cargo_bin("canopa-notify")
        .unwrap()
        .args(["panel-provider", "--socket", support::path_arg(&socket)])
        .assert()
        .success()
        .stdout(contains("Canopa Notify daemon unavailable"))
        .stdout(contains("\"protocol_version\":1"));
}

#[tokio::test]
async fn panel_provider_renders_grouped_notification_history_from_daemon_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");

    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    seed.receive_notification(NotificationInput {
        app_name: "Mail".to_string(),
        summary: "Inbox".to_string(),
        body: "One unread message".to_string(),
        ..NotificationInput::default()
    })
    .await
    .unwrap();
    drop(seed);

    let mut daemon = support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
        "--database",
        support::path_arg(&database),
    ]);

    support::wait_for_status_with_child(&socket, &mut daemon);

    let mut provider = support::canopa_notify_command()
        .args(["panel-provider", "--socket", support::path_arg(&socket)])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let line = read_provider_line(&mut provider);
    let ProviderResponse::ReplaceContent { content, .. } =
        serde_json::from_str::<ProviderResponse>(&line).unwrap();

    assert_eq!(content.sections.len(), 1);
    assert_eq!(content.sections[0].id, "Mail");
    assert_eq!(content.sections[0].items[0].title, "Inbox");
    assert_eq!(
        content.sections[0].items[0].body.as_deref(),
        Some("One unread message")
    );
    assert_eq!(content.sections[0].controls[0].key, "app_permission");
    assert!(line.contains("\"protocol_version\":1"));

    provider.kill().ok();
    provider.wait().ok();
    daemon.kill().ok();
    daemon.wait().ok();
}

fn read_provider_line(provider: &mut std::process::Child) -> String {
    let stdout = provider.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let result = reader.read_line(&mut line).map(|_| line);
        sender.send(result).ok();
    });

    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("panel provider did not emit an initial projection")
        .unwrap()
}
