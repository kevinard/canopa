mod support;

use canopa_panel_protocol::{InteractionEvent, InteractionTarget, PanelContent, ProviderResponse};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use zbus::zvariant::OwnedValue;

struct DaemonProcess(Child);

impl DaemonProcess {
    fn start(socket: &Path, database: &Path, bus_name: &str) -> Self {
        let mut process = Self(support::spawn_canopa_notify([
            "daemon",
            "--headless",
            "--socket",
            support::path_arg(socket),
            "--database",
            support::path_arg(database),
            "--dev-bus-name",
            bus_name,
        ]));
        support::wait_for_status_with_child(socket, &mut process.0);
        process
    }
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

struct Provider {
    _process: tokio::process::Child,
    input: tokio::process::ChildStdin,
    output: Lines<BufReader<tokio::process::ChildStdout>>,
}

impl Provider {
    fn start(socket: &Path) -> Self {
        let mut process = tokio::process::Command::from(support::canopa_notify_command())
            .args(["panel-provider", "--socket", support::path_arg(socket)])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let input = process.stdin.take().unwrap();
        let output = BufReader::new(process.stdout.take().unwrap()).lines();
        Self {
            _process: process,
            input,
            output,
        }
    }

    async fn next(&mut self) -> PanelContent {
        let line = tokio::time::timeout(Duration::from_secs(5), self.output.next_line())
            .await
            .expect("provider update deadline")
            .unwrap()
            .expect("provider EOF");
        let ProviderResponse::ReplaceContent {
            protocol_version,
            content,
        } = serde_json::from_str(&line).unwrap();
        assert_eq!(protocol_version, 3);
        assert!(
            content.error.is_none(),
            "unexpected provider error: {:?}",
            content.error
        );
        content
    }

    async fn invoke(&mut self, id: &str, key: &str, value: Option<String>) {
        let event = InteractionEvent::Invoke {
            target: InteractionTarget::Item { id: id.to_owned() },
            action_key: key.to_owned(),
            value,
        };
        let line = format!("{}\n", serde_json::to_string(&event).unwrap());
        self.input.write_all(line.as_bytes()).await.unwrap();
    }
}

async fn action_signal(signals: &mut zbus::proxy::SignalStream<'_>) -> (u32, String) {
    tokio::time::timeout(Duration::from_secs(3), signals.next())
        .await
        .expect("missing action signal")
        .unwrap()
        .body()
        .deserialize()
        .unwrap()
}

async fn invoke_captured_action(
    socket: &Path,
    item: &canopa_panel_protocol::PanelItem,
    action: &canopa_panel_protocol::PanelAction,
) -> String {
    let event = InteractionEvent::Invoke {
        target: InteractionTarget::Item {
            id: item.id.clone(),
        },
        action_key: action.key.clone(),
        value: action.value.clone(),
    };
    let mut connection = tokio::net::UnixStream::connect(socket).await.unwrap();
    connection
        .write_all(
            format!(
                "panel-interaction {}\n",
                serde_json::to_string(&event).unwrap()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut response = String::new();
    tokio::time::timeout(
        Duration::from_secs(3),
        BufReader::new(connection).read_line(&mut response),
    )
    .await
    .expect("interaction completion deadline")
    .unwrap();
    response
}

#[tokio::test]
async fn captured_center_action_cannot_invoke_a_reused_key_on_replacement_content() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = format!("org.canopa.TransientCenter.Revision{}", std::process::id());
    let _daemon = DaemonProcess::start(&socket, &database, &bus_name);
    let connection = zbus::Connection::session().await.unwrap();
    let proxy = zbus::Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let mut signals = proxy.receive_signal("ActionInvoked").await.unwrap();
    let mut provider = Provider::start(&socket);
    assert!(provider.next().await.sections.is_empty());
    for transient in [false, true] {
        let mut wire_id = 0u32;
        let mut captured: Option<canopa_panel_protocol::PanelItem> = None;
        for summary in ["Original", "Replacement"] {
            wire_id = proxy
                .call(
                    "Notify",
                    &(
                        "Preview",
                        wire_id,
                        "",
                        summary,
                        "",
                        vec!["same", "Open"],
                        HashMap::from([
                            ("transient".to_string(), OwnedValue::from(transient)),
                            ("resident".to_string(), OwnedValue::from(true)),
                        ]),
                        0i32,
                    ),
                )
                .await
                .unwrap();
            let content = provider.next().await;
            let item = content.sections[0]
                .items
                .iter()
                .find(|item| item.title == summary)
                .unwrap();
            if let Some(old) = captured.as_ref() {
                assert_eq!(item.id, old.id, "replacement must preserve item identity");
                let result = invoke_captured_action(&socket, old, &old.actions[0]).await;
                assert!(
                    result.starts_with("error "),
                    "stale action was accepted: {result}"
                );
                assert_eq!(
                    invoke_captured_action(&socket, item, &item.actions[0]).await,
                    "ok\n"
                );
                assert_eq!(action_signal(&mut signals).await, (wire_id, "same".into()));
                provider.next().await;
            } else {
                captured = Some(item.clone());
            }
        }
        proxy
            .call::<_, _, ()>("CloseNotification", &(wire_id,))
            .await
            .unwrap();
        provider.next().await;
    }
}

#[tokio::test]
async fn provider_marks_transient_items_and_routes_every_action_to_the_live_sender() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = format!("org.canopa.TransientCenter.Actions{}", std::process::id());
    let _daemon = DaemonProcess::start(&socket, &database, &bus_name);
    let connection = zbus::Connection::session().await.unwrap();
    let proxy = zbus::Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let mut signals = proxy.receive_signal("ActionInvoked").await.unwrap();
    let wire_id: u32 = proxy
        .call(
            "Notify",
            &(
                "Preview",
                0u32,
                "",
                "Runtime only",
                "<b>Preview</b> body",
                vec![
                    "default",
                    "",
                    "one",
                    "First",
                    "two",
                    "Second",
                    "three",
                    "Third",
                    "canopa-notify:clear-active",
                    "Fourth — opaque sender action",
                ],
                HashMap::from([
                    ("transient".to_string(), OwnedValue::from(true)),
                    ("resident".to_string(), OwnedValue::from(true)),
                ]),
                0i32,
            ),
        )
        .await
        .unwrap();
    let mut provider = Provider::start(&socket);
    let content = provider.next().await;
    assert_eq!(content.sections.len(), 1);
    let item = &content.sections[0].items[0];
    assert!(
        item.metadata
            .iter()
            .any(|entry| entry.key == "retention" && entry.value == "Runtime only")
    );
    assert_eq!(item.body.as_deref(), Some("Preview body"));
    assert!(item.styled_body.as_ref().unwrap().runs[0].style.bold);
    let activation = item
        .actions
        .iter()
        .find(|action| action.key == "canopa-notify:activate-item")
        .expect("the host needs an advertised item-activation control");
    assert_eq!(activation.label, "Open notification");
    let named_actions: Vec<_> = item
        .actions
        .iter()
        .filter(|action| action.key == "canopa-notify:notification-action")
        .collect();
    assert_eq!(
        named_actions
            .iter()
            .map(|action| action.label.as_str())
            .collect::<Vec<_>>(),
        ["First", "Second", "Third", "Fourth — opaque sender action"]
    );
    for (action, sender_key) in
        named_actions
            .into_iter()
            .zip(["one", "two", "three", "canopa-notify:clear-active"])
    {
        provider
            .invoke(&item.id, &action.key, action.value.clone())
            .await;
        assert_eq!(
            action_signal(&mut signals).await,
            (wire_id, sender_key.to_owned())
        );
        let update = provider.next().await;
        assert_eq!(
            update.sections[0].items[0].id, item.id,
            "resident action removed item"
        );
    }
    provider
        .invoke(&item.id, &activation.key, activation.value.clone())
        .await;
    assert_eq!(
        action_signal(&mut signals).await,
        (wire_id, "default".to_string())
    );
    assert_eq!(provider.next().await.sections[0].items[0].id, item.id);
    proxy
        .call::<_, _, ()>("CloseNotification", &(wire_id,))
        .await
        .unwrap();
    assert!(provider.next().await.sections.is_empty());
}

#[tokio::test]
async fn center_targets_from_a_previous_daemon_cannot_act_on_reused_transient_ids() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = format!("org.canopa.TransientCenter.Restart{}", std::process::id());
    let mut captured: Option<canopa_panel_protocol::PanelItem> = None;
    for _ in 0..2 {
        let daemon = DaemonProcess::start(&socket, &database, &bus_name);
        let connection = zbus::Connection::session().await.unwrap();
        let proxy = zbus::Proxy::new(
            &connection,
            bus_name.as_str(),
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        )
        .await
        .unwrap();
        let mut signals = proxy.receive_signal("ActionInvoked").await.unwrap();
        let wire_id: u32 = proxy
            .call(
                "Notify",
                &(
                    "Preview",
                    0u32,
                    "",
                    "Same content and revision",
                    "",
                    vec!["default", "", "same", "Named"],
                    HashMap::from([
                        ("transient".to_string(), OwnedValue::from(true)),
                        ("resident".to_string(), OwnedValue::from(true)),
                    ]),
                    0i32,
                ),
            )
            .await
            .unwrap();
        let mut provider = Provider::start(&socket);
        let content = provider.next().await;
        let item = &content.sections[0].items[0];
        if let Some(old) = captured.as_ref() {
            assert_eq!(item.id, old.id, "fixture must reuse the transient ID");
            assert_ne!(
                item.actions, old.actions,
                "new lifetime must have fresh targets"
            );
            for action in &old.actions {
                assert!(
                    invoke_captured_action(&socket, old, action)
                        .await
                        .starts_with("error ")
                );
            }
            for (action, key) in item.actions.iter().zip(["same", "default"]) {
                provider
                    .invoke(&item.id, &action.key, action.value.clone())
                    .await;
                assert_eq!(action_signal(&mut signals).await, (wire_id, key.into()));
                provider.next().await;
            }
        } else {
            captured = Some(item.clone());
        }
        drop(provider);
        drop(daemon);
    }
}

#[tokio::test]
async fn center_activation_controls_cannot_switch_meaning_after_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = format!(
        "org.canopa.TransientCenter.Activation{}",
        std::process::id()
    );
    let _daemon = DaemonProcess::start(&socket, &database, &bus_name);
    let connection = zbus::Connection::session().await.unwrap();
    let proxy = zbus::Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let mut closures = proxy.receive_signal("NotificationClosed").await.unwrap();
    let mut signals = proxy.receive_signal("ActionInvoked").await.unwrap();
    let mut provider = Provider::start(&socket);
    assert!(provider.next().await.sections.is_empty());
    for transient in [false, true] {
        for had_default in [false, true] {
            let mut wire_id = 0u32;
            let mut captured: Option<canopa_panel_protocol::PanelItem> = None;
            for (summary, has_default) in [("Original", had_default), ("Replacement", !had_default)]
            {
                wire_id = proxy
                    .call(
                        "Notify",
                        &(
                            "Preview",
                            wire_id,
                            "",
                            summary,
                            "",
                            if has_default {
                                vec!["default", ""]
                            } else {
                                vec![]
                            },
                            HashMap::from([("transient".to_string(), OwnedValue::from(transient))]),
                            0i32,
                        ),
                    )
                    .await
                    .unwrap();
                let content = provider.next().await;
                let item = content.sections[0]
                    .items
                    .iter()
                    .find(|item| item.title == summary && !item.actions.is_empty())
                    .unwrap();
                if let Some(old) = captured.as_ref() {
                    assert_eq!(item.id, old.id);
                    assert!(
                        invoke_captured_action(&socket, old, &old.actions[0])
                            .await
                            .starts_with("error ")
                    );
                    assert_eq!(
                        invoke_captured_action(&socket, item, &item.actions[0]).await,
                        "ok\n"
                    );
                    let closure = tokio::time::timeout(Duration::from_secs(3), closures.next())
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(
                        closure.body().deserialize::<(u32, u32)>().unwrap(),
                        (wire_id, 2)
                    );
                    if has_default {
                        assert_eq!(
                            action_signal(&mut signals).await,
                            (wire_id, "default".into())
                        );
                    }
                    let update = provider.next().await;
                    assert!(
                        update
                            .sections
                            .iter()
                            .flat_map(|group| &group.items)
                            .filter(|candidate| candidate.id == item.id)
                            .all(|candidate| candidate.actions.is_empty())
                    );
                    // The completed socket exchange is a barrier: duplicate terminal events
                    // must be rejected, not interpreted against history or another live item.
                    assert!(
                        invoke_captured_action(&socket, item, &item.actions[0])
                            .await
                            .starts_with("error ")
                    );
                } else {
                    captured = Some(item.clone());
                }
            }
        }
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(100), signals.next())
            .await
            .is_err()
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), closures.next())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn provider_advertises_dismissal_for_live_items_but_not_inactive_history() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = format!("org.canopa.TransientCenter.Dismiss{}", std::process::id());
    let _daemon = DaemonProcess::start(&socket, &database, &bus_name);
    let connection = zbus::Connection::session().await.unwrap();
    let proxy = zbus::Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let mut closures = proxy.receive_signal("NotificationClosed").await.unwrap();
    let mut provider = Provider::start(&socket);
    assert!(provider.next().await.sections.is_empty());
    for transient in [true, false] {
        let wire_id: u32 = proxy
            .call(
                "Notify",
                &(
                    "Preview",
                    0u32,
                    "",
                    "Dismiss me",
                    "No default action",
                    Vec::<String>::new(),
                    HashMap::from([("transient".to_string(), OwnedValue::from(transient))]),
                    0i32,
                ),
            )
            .await
            .unwrap();
        let content = provider.next().await;
        let item = &content.sections[0].items[0];
        assert_eq!(item.actions.len(), 1);
        let dismissal = &item.actions[0];
        assert_eq!(dismissal.key, "canopa-notify:activate-item");
        assert_eq!(dismissal.label, "Dismiss notification");
        assert!(item.metadata.iter().any(|entry| entry.key == "retention"
            && entry.value
                == if transient {
                    "Runtime only"
                } else {
                    "Retained"
                }));
        provider
            .invoke(&item.id, &dismissal.key, dismissal.value.clone())
            .await;
        let signal = tokio::time::timeout(Duration::from_secs(3), closures.next())
            .await
            .expect("missing dismissal signal")
            .unwrap();
        assert_eq!(
            signal.body().deserialize::<(u32, u32)>().unwrap(),
            (wire_id, 2)
        );
        let after = provider.next().await;
        if transient {
            assert!(after.sections.is_empty());
        } else {
            let historical = &after.sections[0].items[0];
            assert_eq!(historical.id, item.id);
            assert!(historical.actions.is_empty());
            assert!(
                historical
                    .metadata
                    .iter()
                    .any(|entry| entry.key == "status" && entry.value == "dismissed")
            );
        }
    }
}
