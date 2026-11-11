use crate::support;

use canopa_notify::{Daemon, DaemonOptions};
use canopa_panel_protocol::{PanelContent, ProviderResponse};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use support::notifications::notification_with_body;

struct Process(Child);

impl Drop for Process {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

fn start_daemon(socket: &std::path::Path, database: &std::path::Path) -> Process {
    let mut child = Process(support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(socket),
        "--database",
        support::path_arg(database),
    ]));
    support::wait_for_status_with_child(socket, &mut child.0);
    child
}

fn connect(socket: &std::path::Path, request: &[u8]) -> BufReader<UnixStream> {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(request).unwrap();
    BufReader::new(stream)
}

fn frame(reader: &mut impl BufRead) -> String {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(line.ends_with('\n'), "expected a complete frame");
    line
}

fn accept(listener: &UnixListener) -> BufReader<UnixStream> {
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(8)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(8)))
                    .unwrap();
                return BufReader::new(stream);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "provider did not connect"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("{error}"),
        }
    }
}

fn provider(socket: &std::path::Path) -> (Process, mpsc::Receiver<std::io::Result<String>>) {
    provider_for_protocol(socket, "canopa-v4")
}
fn provider_for_protocol(
    socket: &std::path::Path,
    protocol: &str,
) -> (Process, mpsc::Receiver<std::io::Result<String>>) {
    let mut child = Process(
        support::canopa_notify_command()
            .args([
                "panel-provider",
                "--socket",
                support::path_arg(socket),
                "--protocol",
                protocol,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            let result = reader.read_line(&mut line);
            if matches!(result, Ok(0)) {
                break;
            }
            if sender.send(result.map(|_| line)).is_err() {
                break;
            }
        }
    });
    (child, receiver)
}

fn output(receiver: &mpsc::Receiver<std::io::Result<String>>) -> String {
    receiver
        .recv_timeout(Duration::from_secs(8))
        .expect("provider did not emit a frame")
        .unwrap()
}

fn wire_content(title: &str) -> String {
    serde_json::to_string(&ProviderResponse::replace_content(PanelContent::new(title))).unwrap()
        + "\n"
}

fn interaction(value: &str) -> String {
    serde_json::to_string(&canopa_panel_protocol::InteractionEvent::Invoke {
        target: canopa_panel_protocol::InteractionTarget::Content,
        action_key: "canopa-notify:clear-active".into(),
        value: Some(value.into()),
    })
    .unwrap()
        + "\n"
}

#[test]
fn provider_interaction_replies_are_bounded_and_failures_do_not_expose_payloads() {
    use std::io::Read;
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let (mut provider, frames) = provider(&socket);
    let mut subscription = accept(&listener);
    assert_eq!(frame(&mut subscription), "panel-session\n");
    subscription
        .get_mut()
        .write_all(wire_content("ready").as_bytes())
        .unwrap();
    let _ = output(&frames);
    let replies = [
        b"error private-daemon-response\n".to_vec(),
        format!("private-daemon-response{}", "x".repeat(4097)).into_bytes(),
        format!("{}\n", "x".repeat(4095)).into_bytes(),
        vec![0xff, b'\n'],
        b"ok\n".to_vec(),
    ];
    for reply in replies {
        provider
            .0
            .stdin
            .as_mut()
            .unwrap()
            .write_all(interaction("private-host-value").as_bytes())
            .unwrap();
        let mut request = accept(&listener);
        assert!(frame(&mut request).contains("private-host-value"));
        request
            .get_mut()
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let _ = request.get_mut().write_all(&reply);
        // A complete or over-limit response must release the exchange without EOF.
        let mut byte = [0];
        match request.read(&mut byte) {
            Ok(0) => {}
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
            result => {
                panic!("provider kept reading an already complete/excessive reply: {result:?}")
            }
        }
    }
    drop(subscription);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while provider.0.try_wait().unwrap().is_none() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut diagnostics = String::new();
    provider
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut diagnostics)
        .unwrap();
    assert!(diagnostics.contains("event=interaction_failed"));
    assert!(!diagnostics.contains("private-daemon-response"));
    assert!(!diagnostics.contains("private-host-value"));
    assert!(diagnostics.len() < 1024);
}

#[test]
fn interaction_deadline_is_total_and_releases_serial_forwarding() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let (mut provider, frames) = provider(&socket);
    let mut subscription = accept(&listener);
    assert_eq!(frame(&mut subscription), "panel-session\n");
    subscription
        .get_mut()
        .write_all(wire_content("ready").as_bytes())
        .unwrap();
    let _ = output(&frames);
    provider
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(format!("{}{}", interaction("first"), interaction("second")).as_bytes())
        .unwrap();
    let mut first = accept(&listener);
    assert!(frame(&mut first).contains("first"));
    let started = std::time::Instant::now();
    let slow_peer = std::thread::spawn(move || {
        for _ in 0..200 {
            if first.get_mut().write_all(b"x").is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    let mut second = accept(&listener);
    assert!(
        started.elapsed() < Duration::from_secs(7),
        "partial reads extended the total exchange deadline"
    );
    assert!(
        started.elapsed() >= Duration::from_secs(4),
        "fixture should exercise the exchange deadline"
    );
    assert!(frame(&mut second).contains("second"));
    second.get_mut().write_all(b"ok\n").unwrap();
    slow_peer.join().unwrap();
    subscription
        .get_mut()
        .write_all(wire_content("still connected").as_bytes())
        .unwrap();
    assert_eq!(output(&frames), wire_content("still connected"));
}

#[test]
fn provider_bounds_interaction_lines_and_recovers_after_invalid_input() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let (mut provider, frames) = provider(&socket);
    let mut subscription = accept(&listener);
    assert_eq!(frame(&mut subscription), "panel-session\n");
    subscription
        .get_mut()
        .write_all(wire_content("ready").as_bytes())
        .unwrap();
    let _ = output(&frames);

    let exact = interaction(&"x".repeat(65_518 - interaction("").len()));
    assert_eq!(exact.len(), 65_518);
    let oversized = interaction(&"x".repeat(65_519 - interaction("").len()));
    let valid = interaction("é recovered");
    let mut input = exact.as_bytes().to_vec();
    input.extend_from_slice(oversized.as_bytes());
    input.extend_from_slice(&[0xff, b'\n']);
    input.extend_from_slice(&vec![b'x'; 2 * 1024 * 1024]);
    input.extend_from_slice(b"\nprivate invalid json\n\r\n");
    input.extend_from_slice(valid.replace('\n', "\r\n").as_bytes());
    input.extend_from_slice(interaction("incomplete").trim_end().as_bytes());
    let mut stdin = provider.0.stdin.take().unwrap();
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let mut first = accept(&listener);
    let received = frame(&mut first);
    assert_eq!(received, format!("panel-interaction {exact}"));
    assert_eq!(received.len(), 65_536);
    first.get_mut().write_all(b"ok\n").unwrap();
    drop(first);
    let mut second = accept(&listener);
    let received = frame(&mut second);
    assert!(
        received == format!("panel-interaction {valid}"),
        "oversized or malformed host input reached the daemon ({} bytes)",
        received.len()
    );
    second.get_mut().write_all(b"ok\n").unwrap();
    drop(second);
    writer.join().unwrap().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
        "unterminated final input was forwarded"
    );
}

#[test]
fn provider_validates_complete_frames_and_accepts_the_exact_host_byte_limit() {
    let base = wire_content("é");
    let exact = wire_content(&format!("é{}", "x".repeat(1_048_576 - base.len())));
    assert_eq!(exact.len(), 1_048_576);
    let invalid_content = r#"{"type":"replace_content","protocol_version":4,"content":{"title":"private","sections":[],"actions":[{"key":"","label":"private","value":null}],"error":null}}"#;
    let cases = [
        (exact.as_bytes().to_vec(), true),
        (
            wire_content(&"x".repeat(1_048_577 - wire_content("").len())).into_bytes(),
            false,
        ),
        (b"{private malformed json}\n".to_vec(), false),
        (
            wire_content("private")
                .replace("\"protocol_version\":4", "\"protocol_version\":2")
                .into_bytes(),
            false,
        ),
        (format!("{invalid_content}\n").into_bytes(), false),
        (vec![0xff, b'\n'], false),
        (
            wire_content("private").trim_end().as_bytes().to_vec(),
            false,
        ),
    ];
    for (bytes, valid) in cases {
        let temp = tempfile::tempdir().unwrap();
        let socket = temp.path().join("control.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let (_provider, output_frames) = provider(&socket);
        let mut subscription = accept(&listener);
        assert_eq!(frame(&mut subscription), "panel-session\n");
        subscription
            .get_mut()
            .write_all(wire_content("ready").as_bytes())
            .unwrap();
        assert_eq!(output(&output_frames), wire_content("ready"));
        let _ = subscription.get_mut().write_all(&bytes);
        subscription
            .get_mut()
            .shutdown(std::net::Shutdown::Write)
            .unwrap();
        let received = output(&output_frames);
        if valid {
            assert_eq!(received.as_bytes(), bytes);
        } else {
            assert!(
                received.len() < 1024,
                "invalid frame reached the host: {} bytes",
                received.len()
            );
            let ProviderResponse::ReplaceContent { content, .. } =
                serde_json::from_str(&received).unwrap();
            assert_eq!(
                content.error.as_deref(),
                Some(
                    "Notification Center stream ended with an invalid or incomplete frame; reopen the panel."
                )
            );
            assert!(!received.contains("private"));
        }
    }
}

#[test]
fn panel_subscription_limit_reserves_ordinary_control_capacity() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let _daemon = start_daemon(&socket, &temp.path().join("history.db"));
    let mut panels = Vec::new();
    for _ in 0..8 {
        let mut panel = connect(&socket, b"panel-subscribe\n");
        let ProviderResponse::ReplaceContent { content, .. } =
            serde_json::from_str(&frame(&mut panel)).unwrap();
        assert_eq!(content.error, None);
        panels.push(panel);
    }
    let mut excess = connect(&socket, b"panel-subscribe\n");
    let ProviderResponse::ReplaceContent { content, .. } =
        serde_json::from_str(&frame(&mut excess)).unwrap();
    assert_eq!(
        content.error.as_deref(),
        Some("Too many Notification Center subscriptions; close a panel and retry.")
    );
    assert!(frame(&mut connect(&socket, b"status\n")).contains("available"));
}

#[tokio::test]
async fn installed_provider_pages_large_history_without_losing_content() {
    large_history("canopa-v4").await;
}
#[tokio::test]
async fn installed_v5_provider_pages_large_history_without_losing_content() {
    large_history("canopa-v5").await;
}
async fn large_history(protocol: &str) {
    fn app(index: usize) -> &'static str {
        if index == 23 || index == 7 {
            ""
        } else if index.is_multiple_of(3) {
            "Chat"
        } else {
            "Mail"
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let socket = temp.path().join("control.sock");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    for index in 0..24 {
        let mut input =
            notification_with_body(app(index), format!("Message {index}"), "x".repeat(60_000));
        input.category = Some(
            if index % 2 == 0 {
                "email.arrived"
            } else {
                "email.error"
            }
            .into(),
        );
        input.app_icon =
            canopa_notify::NotificationVisualReference::icon_name(format!("{}-icon", app(index)));
        let receipt = seed.receive_notification(input).await.unwrap();
        if index % 4 == 1 {
            seed.dismiss_notification(receipt.notification_id)
                .await
                .unwrap();
        }
    }
    drop(seed);
    let _daemon = start_daemon(&socket, &database);
    let (mut provider, output) = provider_for_protocol(&socket, protocol);
    let read = || {
        let line = output
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .unwrap();
        assert!(line.len() <= canopa_panel_protocol::MAX_FRAME_BYTES);
        let (content, revision) = if protocol == "canopa-v5" {
            let response = canopa_panel_protocol::v5::Response::decode(line.as_bytes()).unwrap();
            let canopa_panel_protocol::v5::Content::Rich(content) = response.content else {
                panic!("rich content")
            };
            (content, Some(response.revision))
        } else {
            let ProviderResponse::ReplaceContent { content, .. } =
                serde_json::from_str(&line).unwrap();
            (content, None)
        };
        assert!(
            content.error.is_none(),
            "large history needs navigable pages: {:?}",
            content.error
        );
        content.validate().unwrap();
        (content, revision)
    };
    let mut send = |content: &PanelContent, revision, label: &str| {
        let action = content
            .actions
            .iter()
            .find(|action| action.label == label)
            .unwrap();
        let event = canopa_panel_protocol::InteractionEvent::Invoke {
            target: canopa_panel_protocol::InteractionTarget::Content,
            action_key: action.key.clone(),
            value: action.value.clone(),
        };
        let bytes = match revision {
            Some(revision) => canopa_panel_protocol::v5::HostMessage::event(
                revision,
                canopa_panel_protocol::v5::Interaction::Rich(event),
            )
            .encode()
            .unwrap(),
            None => (serde_json::to_string(&event).unwrap() + "\n").into_bytes(),
        };
        provider
            .0
            .stdin
            .as_mut()
            .unwrap()
            .write_all(&bytes)
            .unwrap();
    };
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..24 {
        let (content, revision) = read();
        assert_eq!(content.title, "Notifications");
        assert!(
            content
                .sections
                .iter()
                .all(|section| section.controls.is_empty())
        );
        assert!(
            !content
                .actions
                .iter()
                .any(|action| action.key == "canopa-notify:delete-history")
        );
        send(&content, revision, "Settings");
        let (settings, revision) = read();
        assert_eq!(settings.title, "Notification settings");
        assert!(
            settings
                .sections
                .iter()
                .all(|section| section.items.is_empty())
        );
        assert!(
            settings
                .actions
                .iter()
                .any(|action| action.key == "canopa-notify:delete-history")
        );
        assert_eq!(
            content.sections.len(),
            1,
            "reading order must not be split by app"
        );
        assert!(
            !settings.sections.iter().any(|section| section.id == "app/"),
            "an absent app identity cannot own a persistent rule"
        );
        let mut categories_by_app =
            std::collections::BTreeMap::<_, std::collections::BTreeSet<_>>::new();
        for item in &content.sections[0].items {
            let index: usize = item
                .title
                .strip_prefix("Message ")
                .unwrap()
                .parse()
                .unwrap();
            if !app(index).is_empty() {
                categories_by_app.entry(app(index)).or_default().insert(
                    if index.is_multiple_of(2) {
                        "email.arrived"
                    } else {
                        "email.error"
                    },
                );
            }
        }
        // Settings starts with the Do Not Disturb header (ADR 0055), then one section per app.
        assert_eq!(settings.sections[0].id, "do-not-disturb");
        assert_eq!(settings.sections.len(), categories_by_app.len() + 1);
        for (app, categories) in categories_by_app {
            let section = settings
                .sections
                .iter()
                .find(|section| section.id == format!("app/{app}"))
                .unwrap();
            assert_eq!(section.title, app);
            assert!(
                matches!(&section.icon, Some(canopa_panel_protocol::PanelIcon::Theme { names, .. }) if names == &[format!("{app}-icon")])
            );
            assert_eq!(
                section.controls.len(),
                categories.len() + 1,
                "category controls must be deduplicated and page-local"
            );
            for category in categories {
                assert!(
                    section
                        .controls
                        .iter()
                        .any(|control| control.key.ends_with(category)),
                    "missing category permission for {category}"
                );
            }
        }
        send(&settings, revision, "Back to notifications");
        let (returned, revision) = read();
        assert_eq!(
            returned.sections, content.sections,
            "settings must preserve the reading position"
        );
        let content = returned;
        for item in content.sections.iter().flat_map(|section| &section.items) {
            assert_eq!(
                item.title,
                format!("Message {}", 23 - seen.len()),
                "newest-first order must survive interleaved apps, attention states and page boundaries"
            );
            assert!(
                seen.insert(item.title.clone()),
                "a stable notification appeared on two pages"
            );
            let body = item.rich_body.as_ref().unwrap();
            assert_eq!(
                body.segments
                    .iter()
                    .filter_map(|segment| match segment {
                        canopa_panel_protocol::RichBodySegment::Text { text, .. } =>
                            Some(text.len()),
                        _ => None,
                    })
                    .sum::<usize>(),
                60_000
            );
        }
        if !content
            .actions
            .iter()
            .any(|action| action.label == "Next page")
        {
            break;
        }
        send(&content, revision, "Next page");
    }
    assert_eq!(
        seen.len(),
        24,
        "every retained notification must be reachable"
    );
}

#[tokio::test]
async fn paginated_views_navigate_independently_and_reject_stale_navigation() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let socket = temp.path().join("control.sock");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    for index in 0..36 {
        seed.receive_notification(notification_with_body(
            "Mail",
            format!("Message {index}"),
            "x".repeat(60_000),
        ))
        .await
        .unwrap();
    }
    drop(seed);
    let _daemon = start_daemon(&socket, &database);
    let mut a = connect(&socket, b"panel-session\n");
    let mut b = connect(&socket, b"panel-session\n");
    let read = |reader: &mut BufReader<UnixStream>| {
        let line = frame(reader);
        assert!(line.len() <= canopa_panel_protocol::MAX_FRAME_BYTES);
        let ProviderResponse::ReplaceContent { content, .. } = serde_json::from_str(&line).unwrap();
        assert!(content.error.is_none());
        content
    };
    let send = |reader: &mut BufReader<UnixStream>, content: &PanelContent, label: &str| {
        let action = content
            .actions
            .iter()
            .find(|action| action.label == label)
            .unwrap();
        let event = canopa_panel_protocol::InteractionEvent::Invoke {
            target: canopa_panel_protocol::InteractionTarget::Content,
            action_key: action.key.clone(),
            value: action.value.clone(),
        };
        writeln!(
            reader.get_mut(),
            "{}",
            serde_json::to_string(&event).unwrap()
        )
        .unwrap();
    };
    let titles = |content: &PanelContent| {
        content
            .sections
            .iter()
            .flat_map(|section| &section.items)
            .map(|item| item.title.clone())
            .collect::<Vec<_>>()
    };
    let first = read(&mut a);
    let independent_first = read(&mut b);
    assert_eq!(titles(&first), titles(&independent_first));
    send(&mut a, &first, "Settings");
    let settings = read(&mut a);
    assert_eq!(settings.title, "Notification settings");
    assert!(titles(&settings).is_empty());
    // An obsolete view change cannot consume the current Back event.
    send(&mut a, &first, "Settings");
    send(&mut a, &settings, "Back to notifications");
    let first = read(&mut a);
    assert_eq!(titles(&first), titles(&independent_first));
    send(&mut a, &first, "Next page");
    let second = read(&mut a);
    assert_ne!(titles(&first), titles(&second));
    assert!(
        second
            .actions
            .iter()
            .any(|action| action.label == "Next page")
    );
    // Queue an obsolete Next and then a current Previous as a processing barrier.
    // The obsolete action must not advance the generation or consume the new one.
    send(&mut a, &first, "Next page");
    send(&mut a, &second, "Previous page");
    let returned = read(&mut a);
    assert_eq!(titles(&returned), titles(&first));
    send(&mut b, &independent_first, "Next page");
    assert_eq!(titles(&read(&mut b)), titles(&second));
    send(&mut a, &returned, "Next page");
    let second = read(&mut a);
    send(&mut a, &second, "First page");
    assert_eq!(titles(&read(&mut a)), titles(&first));
}

#[tokio::test]
async fn oversized_panel_projection_reports_error_and_recovers_without_deleting_history() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let socket = temp.path().join("control.sock");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    for _ in 0..12 {
        seed.receive_notification(notification_with_body(
            "Mail",
            "Private summary",
            "x".repeat(60_000),
        ))
        .await
        .unwrap();
    }
    drop(seed);
    let _daemon = start_daemon(&socket, &database);
    let mut subscription = connect(&socket, b"panel-subscribe\n");
    let line = frame(&mut subscription);
    assert!(
        line.len() <= 1_048_576,
        "provider emitted an oversized frame: {} bytes",
        line.len()
    );
    let ProviderResponse::ReplaceContent { content, .. } = serde_json::from_str(&line).unwrap();
    assert_eq!(
        content.error.as_deref(),
        Some(
            "Notification Center exceeds the 1 MiB display limit; use notification history controls."
        )
    );
    assert!(content.sections.is_empty());
    assert!(!line.contains("Private summary"));
    let mut snapshot = connect(&socket, b"panel-snapshot\n");
    assert_eq!(frame(&mut snapshot), line);

    // The transport error never changes notification history.
    let mut history = connect(
        &socket,
        b"control {\"command\":\"notification-center-projection\"}\n",
    );
    let reply: serde_json::Value = serde_json::from_str(&frame(&mut history)).unwrap();
    assert_eq!(
        reply["value"]["groups"][0]["items"]
            .as_array()
            .unwrap()
            .len(),
        12
    );
    let mut clear = connect(
        &socket,
        b"control {\"command\":\"delete-notification-history\"}\n",
    );
    let _ = frame(&mut clear);
    let ProviderResponse::ReplaceContent { content, .. } =
        serde_json::from_str(&frame(&mut subscription)).unwrap();
    assert_eq!(content.error, None);
    assert!(content.sections.is_empty());
}

#[test]
fn closed_or_misused_idle_subscriptions_release_slots_without_state_changes() {
    use std::io::Read;
    use std::net::Shutdown;
    use std::time::Instant;
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let _daemon = start_daemon(&socket, &temp.path().join("history.db"));
    for round in 0..80 {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut panel = loop {
            let mut panel = connect(&socket, b"panel-subscribe\n");
            let ProviderResponse::ReplaceContent { content, .. } =
                serde_json::from_str(&frame(&mut panel)).unwrap();
            if content.error.is_none() {
                break panel;
            }
            assert!(
                Instant::now() < deadline,
                "closed subscriptions retained all slots after {round} openings"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        match round % 3 {
            0 => drop(panel),
            1 => {
                panel.get_mut().shutdown(Shutdown::Write).unwrap();
                let mut byte = [0];
                assert_eq!(panel.read(&mut byte).unwrap(), 0);
            }
            _ => {
                panel
                    .get_mut()
                    .write_all(b"unexpected follow-up\n")
                    .unwrap();
                let mut byte = [0];
                assert_eq!(panel.read(&mut byte).unwrap(), 0);
            }
        }
    }
    // Extra input already buffered alongside the initial request is also terminal.
    let mut pipelined = connect(&socket, b"panel-subscribe\nstatus\n");
    let mut remaining = String::new();
    pipelined.read_to_string(&mut remaining).unwrap();
    assert!(
        remaining.is_empty(),
        "one-way subscriptions must reject pipelined commands"
    );
    assert!(frame(&mut connect(&socket, b"status\n")).contains("available"));
}

#[tokio::test]
async fn stalled_socket_writes_expire_but_healthy_idle_panels_keep_receiving_updates() {
    use std::io::Read;
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    for _ in 0..4 {
        seed.receive_notification(notification_with_body("Mail", "Inbox", "x".repeat(60_000)))
            .await
            .unwrap();
    }
    drop(seed);
    let _daemon = start_daemon(&socket, &database);
    let mut stalled = connect(&socket, b"panel-subscribe\n");
    let mut healthy = connect(&socket, b"panel-subscribe\n");
    let initial = frame(&mut healthy);
    assert!(initial.len() > 700_000 && initial.len() < 1_048_576);
    // Keep every subscription slot occupied until the stalled write deadline.
    let mut other_panels = Vec::new();
    for _ in 0..6 {
        let mut panel = connect(&socket, b"panel-subscribe\n");
        let _ = frame(&mut panel);
        other_panels.push(panel);
    }
    std::thread::sleep(Duration::from_secs(6));
    let mut reopened = connect(&socket, b"panel-subscribe\n");
    let ProviderResponse::ReplaceContent { content, .. } =
        serde_json::from_str(&frame(&mut reopened)).unwrap();
    assert_eq!(
        content.error, None,
        "a stalled write retained subscription admission"
    );
    let mut partial = Vec::new();
    stalled.read_to_end(&mut partial).unwrap();
    assert!(
        partial.len() < initial.len(),
        "fixture must exceed the unread socket capacity"
    );
    assert!(
        !partial.ends_with(b"\n"),
        "timed-out partial frames must not be repaired or reused"
    );

    let mut clear = connect(
        &socket,
        b"control {\"command\":\"delete-notification-history\"}\n",
    );
    let _ = frame(&mut clear);
    let ProviderResponse::ReplaceContent { content, .. } =
        serde_json::from_str(&frame(&mut healthy)).unwrap();
    assert_eq!(content.error, None);
    assert!(
        content.sections.is_empty(),
        "healthy idle panel lost its subscription"
    );
}

#[test]
fn v5_provider_rejects_retired_actions_and_exits_on_close_or_stdin_eof() {
    use canopa_panel_protocol::{v4, v5};
    for explicit_close in [true, false] {
        let temp = tempfile::tempdir().unwrap();
        let socket = temp.path().join("control.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let (mut provider, output_rx) = provider_for_protocol(&socket, "canopa-v5");
        let mut navigation = accept(&listener);
        assert_eq!(frame(&mut navigation), "panel-session\n");
        let mut content = PanelContent::new("First");
        content.actions.push(v4::PanelAction {
            request_activation_token: false,
            key: "opaque-action".into(),
            label: "Invoke".into(),
            value: Some("same-domain-value".into()),
        });
        let send_content = |stream: &mut BufReader<UnixStream>, content| {
            writeln!(
                stream.get_mut(),
                "{}",
                serde_json::to_string(&ProviderResponse::replace_content(content)).unwrap()
            )
            .unwrap();
        };
        send_content(&mut navigation, content.clone());
        let first = v5::Response::decode(output(&output_rx).as_bytes()).unwrap();
        content.title = "Replacement".into();
        send_content(&mut navigation, content);
        let second = v5::Response::decode(output(&output_rx).as_bytes()).unwrap();
        assert!(second.revision > first.revision);
        let invoke = |revision| {
            v5::HostMessage::event(
                revision,
                v5::Interaction::Rich(v4::InteractionEvent::Invoke {
                    target: v4::InteractionTarget::Content,
                    action_key: "opaque-action".into(),
                    value: Some("same-domain-value".into()),
                }),
            )
            .encode()
            .unwrap()
        };
        let mut bytes = invoke(first.revision);
        bytes.extend(invoke(second.revision));
        provider
            .0
            .stdin
            .as_mut()
            .unwrap()
            .write_all(&bytes)
            .unwrap();
        let mut accepted = accept(&listener);
        assert!(frame(&mut accepted).starts_with("panel-interaction "));
        accepted.get_mut().write_all(b"ok\n").unwrap();
        if explicit_close {
            provider
                .0
                .stdin
                .as_mut()
                .unwrap()
                .write_all(&v5::HostMessage::close().encode().unwrap())
                .unwrap();
        } else {
            drop(provider.0.stdin.take());
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(status) = provider.0.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "provider did not exit on close"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "only the current event reaches the daemon"
        );
    }
}
