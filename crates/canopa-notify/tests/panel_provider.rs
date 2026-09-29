mod support;

use assert_cmd::Command;
use canopa_notify::{Daemon, DaemonOptions};
use canopa_panel_protocol::{ProviderResponse, RichBodyImageSource, RichBodySegment};
use predicates::str::contains;
use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use support::notifications::notification_with_body;

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
        .stdout(contains("\"protocol_version\":4"));
}

#[tokio::test]
async fn panel_provider_renders_notification_history_from_daemon_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");

    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    let mut message = notification_with_body("Mail", "Inbox", "<b>One</b> unread message");
    message.app_icon = canopa_notify::NotificationVisualReference::icon_name("mail-unread");
    seed.receive_notification(message).await.unwrap();
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
    let ProviderResponse::ReplaceContent {
        protocol_version,
        content,
    } = serde_json::from_str::<ProviderResponse>(&line).unwrap();

    assert_eq!(protocol_version, 4);

    assert_eq!(content.sections.len(), 1);
    assert_eq!(content.sections[0].id, "notifications");
    assert_eq!(content.sections[0].items[0].title, "Inbox");
    let source = content.sections[0].items[0].attribution.as_ref().unwrap();
    assert_eq!(source.label, "Mail");
    assert!(
        matches!(&source.icon, Some(canopa_panel_protocol::PanelIcon::Theme { names, theme, scale })
        if names == &["mail-unread"] && theme == "hicolor" && *scale == 1)
    );
    let metadata = &content.sections[0].items[0].metadata;
    assert_eq!(
        metadata.len(),
        1,
        "cards carry only the attention indicator"
    );
    assert_eq!(metadata[0].label, "Unread");
    assert!(metadata[0].value.is_empty());
    let body = content.sections[0].items[0].rich_body.as_ref().unwrap();
    assert!(
        matches!(&body.segments[0], RichBodySegment::Text { text, style } if text == "One" && style.bold)
    );
    assert_eq!(
        body.segments
            .iter()
            .filter_map(|segment| match segment {
                RichBodySegment::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>(),
        "One unread message"
    );
    assert!(content.sections[0].controls.is_empty());
    assert!(
        content
            .actions
            .iter()
            .any(|action| action.label == "Settings")
    );
    assert!(line.contains("\"protocol_version\":4"));

    provider.kill().ok();
    provider.wait().ok();
    daemon.kill().ok();
    daemon.wait().ok();
}

#[tokio::test]
async fn panel_provider_exports_rich_body_content_from_daemon_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let body_image_path = temp.path().join("chart.png");

    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    seed.receive_notification(notification_with_body(
        "Reports",
        "Daily report",
        format!(
            r#"<b>Open</b> <img src="{}" alt="chart"/> done <a href="https://example.org/private?token=secret">Read online</a>"#,
            support::path_arg(&body_image_path)
        ),
    ))
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
    let ProviderResponse::ReplaceContent {
        protocol_version,
        content,
    } = serde_json::from_str::<ProviderResponse>(&line).unwrap();

    let item = &content.sections[0].items[0];
    let rich_body = item.rich_body.as_ref().expect("expected rich body");

    assert_eq!(protocol_version, 4);
    assert_eq!(
        rich_body
            .segments
            .iter()
            .filter_map(|segment| match segment {
                RichBodySegment::Text { text, .. } => Some(text.as_str()),
                RichBodySegment::Image { image } => image.alt.as_deref(),
                RichBodySegment::Link { action, .. } => Some(action.label.as_str()),
            })
            .collect::<String>(),
        "Open chart done Read online"
    );
    assert_eq!(rich_body.segments.len(), 5);
    let RichBodySegment::Link { action, uri, .. } = &rich_body.segments[4] else {
        panic!("expected link action")
    };
    assert_eq!(action.label, "Read online");
    assert_eq!(action.key, "canopa-notify:open-link");
    assert!(matches!(
        serde_json::from_str::<canopa_notify::ControlCommand>(action.value.as_deref().unwrap())
            .unwrap(),
        canopa_notify::ControlCommand::InteractWithNotificationCenter {
            action: canopa_notify::NotificationCenterAction::OpenLink { segment_index: 4 },
            ..
        }
    ));
    assert_eq!(uri, "https://example.org/private?token=secret");
    assert!(
        !action.value.as_deref().unwrap().contains("https://"),
        "activation remains revision-scoped and opaque"
    );
    assert!(matches!(
        &rich_body.segments[0],
        RichBodySegment::Text { text, style } if text == "Open" && style.bold
    ));
    assert!(matches!(
        &rich_body.segments[2],
        RichBodySegment::Image { image }
            if image.alt.as_deref() == Some("chart")
                && matches!(
                    &image.source,
                    RichBodyImageSource::LocalPath { path } if path == support::path_arg(&body_image_path)
                )
    ));
    assert!(!line.contains("<img"));

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
