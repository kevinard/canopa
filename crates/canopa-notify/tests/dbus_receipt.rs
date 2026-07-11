mod support;

use assert_cmd::Command;
use canopa_notify::{Daemon, DaemonOptions, DeliveryOutcome, Urgency, UserRule};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, Proxy};

#[tokio::test]
async fn dev_dbus_notify_call_creates_active_notification_visible_through_control_json() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = test_bus_name("Initial");

    let mut daemon = support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
        "--database",
        support::path_arg(&database),
        "--dev-bus-name",
        bus_name.as_str(),
    ]);

    support::wait_for_status_with_child(&socket, &mut daemon);

    let connection = Connection::session().await.unwrap();
    let proxy = Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();

    let mut hints: HashMap<String, OwnedValue> = HashMap::new();
    hints.insert(
        "desktop-entry".to_string(),
        OwnedValue::try_from(Value::from("example-mail.desktop")).unwrap(),
    );
    hints.insert("resident".to_string(), OwnedValue::from(true));

    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Example Mail",
                0u32,
                "",
                "Inbox",
                "<b>One</b> unread message",
                vec![
                    "".to_string(),
                    "Empty key".to_string(),
                    "open".to_string(),
                    "  Open inbox  ".to_string(),
                    "open".to_string(),
                    "Second open".to_string(),
                    "tail".to_string(),
                ],
                hints,
                -1i32,
            ),
        )
        .await
        .unwrap();

    assert!(id > 0);

    let (name, vendor, _version, spec_version): (String, String, String, String) =
        proxy.call("GetServerInformation", &()).await.unwrap();
    assert_eq!(name, "Canopa Notify");
    assert_eq!(vendor, "Canopa");
    assert_eq!(spec_version, "1.3");

    let capabilities: Vec<String> = proxy.call("GetCapabilities", &()).await.unwrap();
    assert_eq!(
        capabilities,
        vec![
            "actions",
            "action-icons",
            "body",
            "body-markup",
            "icon-static",
        ]
    );

    let output = Command::cargo_bin("canopa-notify")
        .unwrap()
        .args([
            "ctl",
            "--socket",
            support::path_arg(&socket),
            "active",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.contains("example-mail.desktop"));
    assert!(json.contains("Inbox"));
    assert!(json.contains("\"body\":\"One unread message\""));
    assert!(json.contains("\"body_content\""));
    assert!(json.contains("\"bold\":true"));
    assert!(json.contains("\"resident\":true"));
    assert!(json.contains("\"key\":\"open\""));
    assert!(json.contains("\"label\":\"Open inbox\""));
    assert!(!json.contains("Empty key"));
    assert!(!json.contains("Second open"));

    daemon.kill().ok();
    daemon.wait().ok();
}

#[tokio::test]
async fn dev_dbus_action_icons_hint_preserves_action_icon_names() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = test_bus_name("ActionIcons");

    let mut daemon = support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
        "--database",
        support::path_arg(&database),
        "--dev-bus-name",
        bus_name.as_str(),
    ]);

    support::wait_for_status_with_child(&socket, &mut daemon);

    let connection = Connection::session().await.unwrap();
    let proxy = Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let hints = HashMap::from([("action-icons".to_string(), OwnedValue::from(true))]);

    proxy
        .call::<_, _, u32>(
            "Notify",
            &(
                "Example Mail",
                0u32,
                "",
                "Inbox",
                "One unread message",
                vec![
                    "mail-reply-sender".to_string(),
                    "Reply".to_string(),
                    "mail-archive".to_string(),
                    "Archive".to_string(),
                ],
                hints,
                -1i32,
            ),
        )
        .await
        .unwrap();

    let output = Command::cargo_bin("canopa-notify")
        .unwrap()
        .args([
            "ctl",
            "--socket",
            support::path_arg(&socket),
            "active",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.contains("\"key\":\"mail-reply-sender\""));
    assert!(json.contains("\"label\":\"Reply\""));
    assert!(json.contains("\"icon\":{\"kind\":\"icon_name\",\"value\":\"mail-reply-sender\"}"));

    daemon.kill().ok();
    daemon.wait().ok();
}

#[tokio::test]
async fn dev_dbus_notify_accepts_body_image_markup_without_advertising_capability() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let body_image_path = temp.path().join("chart.png");
    let bus_name = test_bus_name("BodyImages");

    let mut daemon = support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
        "--database",
        support::path_arg(&database),
        "--dev-bus-name",
        bus_name.as_str(),
    ]);

    support::wait_for_status_with_child(&socket, &mut daemon);

    let connection = Connection::session().await.unwrap();
    let proxy = Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();

    let capabilities: Vec<String> = proxy.call("GetCapabilities", &()).await.unwrap();
    assert!(
        capabilities
            .iter()
            .any(|capability| capability == "body-markup")
    );
    assert!(
        !capabilities
            .iter()
            .any(|capability| capability == "body-images")
    );

    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Reporter",
                0u32,
                "",
                "Report",
                format!(
                    r#"<b>See</b> <img src="{}" alt="chart"/>"#,
                    support::path_arg(&body_image_path)
                ),
                Vec::<String>::new(),
                HashMap::<String, OwnedValue>::new(),
                -1i32,
            ),
        )
        .await
        .unwrap();

    assert!(id > 0);

    let output = Command::cargo_bin("canopa-notify")
        .unwrap()
        .args([
            "ctl",
            "--socket",
            support::path_arg(&socket),
            "active",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json = String::from_utf8(output.stdout).unwrap();

    assert!(json.contains("\"body\":\"See chart\""));
    assert!(json.contains("\"body_content\""));
    assert!(json.contains("\"kind\":\"image\""));
    assert!(json.contains("\"kind\":\"local_path\""));
    assert!(json.contains(support::path_arg(&body_image_path)));
    assert!(json.contains("\"alt\":\"chart\""));
    assert!(json.contains("\"bold\":true"));

    daemon.kill().ok();
    daemon.wait().ok();
}

#[tokio::test]
async fn dev_dbus_notify_replacement_returns_existing_id_and_updates_active_notification() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = test_bus_name("Replacement");

    let mut daemon = support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
        "--database",
        support::path_arg(&database),
        "--dev-bus-name",
        bus_name.as_str(),
    ]);

    support::wait_for_status_with_child(&socket, &mut daemon);

    let connection = Connection::session().await.unwrap();
    let proxy = Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();

    let first_id: u32 = proxy
        .call(
            "Notify",
            &(
                "Downloader",
                0u32,
                "",
                "Download",
                "Starting",
                Vec::<String>::new(),
                HashMap::<String, OwnedValue>::new(),
                -1i32,
            ),
        )
        .await
        .unwrap();

    let replacement_id: u32 = proxy
        .call(
            "Notify",
            &(
                "Downloader",
                first_id,
                "",
                "Download",
                "Complete",
                Vec::<String>::new(),
                HashMap::<String, OwnedValue>::new(),
                -1i32,
            ),
        )
        .await
        .unwrap();

    assert_eq!(replacement_id, first_id);

    let output = Command::cargo_bin("canopa-notify")
        .unwrap()
        .args([
            "ctl",
            "--socket",
            support::path_arg(&socket),
            "active",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.contains("Complete"));
    assert!(!json.contains("Starting"));

    daemon.kill().ok();
    daemon.wait().ok();
}

#[tokio::test]
async fn dev_dbus_notify_fixed_replacement_token_updates_prior_notification() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = test_bus_name("FixedReplacement");

    let mut daemon = support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
        "--database",
        support::path_arg(&database),
        "--dev-bus-name",
        bus_name.as_str(),
    ]);

    support::wait_for_status_with_child(&socket, &mut daemon);

    let connection = Connection::session().await.unwrap();
    let proxy = Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();

    let first_id: u32 = proxy
        .call(
            "Notify",
            &(
                "notify-send",
                9999u32,
                "",
                "Test Progress",
                "0%",
                Vec::<String>::new(),
                HashMap::<String, OwnedValue>::new(),
                -1i32,
            ),
        )
        .await
        .unwrap();
    assert_eq!(first_id, 9999);

    let replacement_id: u32 = proxy
        .call(
            "Notify",
            &(
                "notify-send",
                9999u32,
                "",
                "Test Progress",
                "5%",
                Vec::<String>::new(),
                HashMap::<String, OwnedValue>::new(),
                -1i32,
            ),
        )
        .await
        .unwrap();

    assert_eq!(replacement_id, 9999);

    let output = Command::cargo_bin("canopa-notify")
        .unwrap()
        .args([
            "ctl",
            "--socket",
            support::path_arg(&socket),
            "active",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.contains("5%"));
    assert!(!json.contains("0%"));

    let mut closed_signals = proxy.receive_signal("NotificationClosed").await.unwrap();
    proxy
        .call::<_, _, ()>("CloseNotification", &(9999u32))
        .await
        .unwrap();

    let signal = tokio::time::timeout(std::time::Duration::from_secs(1), closed_signals.next())
        .await
        .unwrap()
        .unwrap();
    let (closed_id, reason): (u32, u32) = signal.body().deserialize().unwrap();
    assert_eq!(closed_id, 9999);
    assert_eq!(reason, 3);

    daemon.kill().ok();
    daemon.wait().ok();
}

#[tokio::test]
async fn dev_dbus_noop_notification_returns_nonzero_sender_id_without_history() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = test_bus_name("Noop");

    let mut daemon = support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
        "--database",
        support::path_arg(&database),
        "--dev-bus-name",
        bus_name.as_str(),
    ]);

    support::wait_for_status_with_child(&socket, &mut daemon);

    let connection = Connection::session().await.unwrap();
    let proxy = Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let mut closed_signals = proxy.receive_signal("NotificationClosed").await.unwrap();

    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Blank Sender",
                0u32,
                "",
                "",
                "",
                Vec::<String>::new(),
                HashMap::<String, OwnedValue>::new(),
                -1i32,
            ),
        )
        .await
        .unwrap();

    assert!(id > 0);
    let signal = tokio::time::timeout(std::time::Duration::from_secs(1), closed_signals.next())
        .await
        .unwrap()
        .unwrap();
    let (closed_id, reason): (u32, u32) = signal.body().deserialize().unwrap();
    assert_eq!((closed_id, reason), (id, 4));

    let close_result: zbus::Result<()> = proxy.call("CloseNotification", &(id,)).await;
    assert!(close_result.is_err());

    let output = Command::cargo_bin("canopa-notify")
        .unwrap()
        .args([
            "ctl",
            "--socket",
            support::path_arg(&socket),
            "active",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(!json.contains("Blank Sender"));

    daemon.kill().ok();
    daemon.wait().ok();
}

#[tokio::test]
async fn dev_dbus_transient_notification_expires_without_history() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = test_bus_name("Transient");
    let mut daemon = support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
        "--database",
        support::path_arg(&database),
        "--dev-bus-name",
        bus_name.as_str(),
    ]);
    support::wait_for_status_with_child(&socket, &mut daemon);

    let connection = Connection::session().await.unwrap();
    let proxy = Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let mut closed_signals = proxy.receive_signal("NotificationClosed").await.unwrap();
    let hints = HashMap::from([("transient".to_string(), OwnedValue::from(true))]);

    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Screenshot",
                0u32,
                "",
                "Captured",
                "Runtime only",
                Vec::<String>::new(),
                hints,
                0i32,
            ),
        )
        .await
        .unwrap();
    let replacement_id: u32 = proxy
        .call(
            "Notify",
            &(
                "Screenshot",
                id,
                "",
                "Captured again",
                "Still runtime only",
                Vec::<String>::new(),
                HashMap::from([("transient".to_string(), OwnedValue::from(true))]),
                50i32,
            ),
        )
        .await
        .unwrap();
    assert_eq!(replacement_id, id);
    let signal = tokio::time::timeout(std::time::Duration::from_secs(1), closed_signals.next())
        .await
        .unwrap()
        .unwrap();
    let (closed_id, reason): (u32, u32) = signal.body().deserialize().unwrap();
    assert_eq!((closed_id, reason), (id, 1));

    let output = Command::cargo_bin("canopa-notify")
        .unwrap()
        .args([
            "ctl",
            "--socket",
            support::path_arg(&socket),
            "active",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        !String::from_utf8(output.stdout)
            .unwrap()
            .contains("Captured")
    );
    let close_result: zbus::Result<()> = proxy.call("CloseNotification", &(id,)).await;
    assert!(close_result.is_err());

    daemon.kill().ok();
    daemon.wait().ok();
}

#[tokio::test]
async fn dev_dbus_policy_drop_closes_reserved_sender_id_without_history() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = test_bus_name("PolicyDrop");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    seed.upsert_user_rule(UserRule::for_app("Blocked", DeliveryOutcome::Dropped))
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
        "--dev-bus-name",
        bus_name.as_str(),
    ]);
    support::wait_for_status_with_child(&socket, &mut daemon);

    let connection = Connection::session().await.unwrap();
    let proxy = Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let mut closed_signals = proxy.receive_signal("NotificationClosed").await.unwrap();

    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Blocked",
                0u32,
                "",
                "Dropped",
                "Not retained",
                Vec::<String>::new(),
                HashMap::<String, OwnedValue>::new(),
                0i32,
            ),
        )
        .await
        .unwrap();
    assert!(id > 0);

    let signal = tokio::time::timeout(std::time::Duration::from_secs(1), closed_signals.next())
        .await
        .unwrap()
        .unwrap();
    let (closed_id, reason): (u32, u32) = signal.body().deserialize().unwrap();
    assert_eq!((closed_id, reason), (id, 4));
    let close_result: zbus::Result<()> = proxy.call("CloseNotification", &(id,)).await;
    assert!(close_result.is_err());

    daemon.kill().ok();
    daemon.wait().ok();
}

#[tokio::test]
async fn dev_dbus_dropped_replacement_preserves_original_sender_lifecycle() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = test_bus_name("DroppedReplacement");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    seed.upsert_user_rule(UserRule::for_app_urgency(
        "Chat",
        Urgency::Normal,
        DeliveryOutcome::Dropped,
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
        "--dev-bus-name",
        bus_name.as_str(),
    ]);
    support::wait_for_status_with_child(&socket, &mut daemon);

    let connection = Connection::session().await.unwrap();
    let proxy = Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let critical_hints = HashMap::from([("urgency".to_string(), OwnedValue::from(2u8))]);
    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Chat",
                0u32,
                "",
                "Original",
                "Keep this body",
                Vec::<String>::new(),
                critical_hints,
                0i32,
            ),
        )
        .await
        .unwrap();
    let mut closed_signals = proxy.receive_signal("NotificationClosed").await.unwrap();

    let replacement_id: u32 = proxy
        .call(
            "Notify",
            &(
                "Chat",
                id,
                "",
                "Replacement",
                "Must not overwrite",
                Vec::<String>::new(),
                HashMap::<String, OwnedValue>::new(),
                0i32,
            ),
        )
        .await
        .unwrap();
    assert_eq!(replacement_id, id);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), closed_signals.next())
            .await
            .is_err()
    );

    let output = Command::cargo_bin("canopa-notify")
        .unwrap()
        .args([
            "ctl",
            "--socket",
            support::path_arg(&socket),
            "active",
            "--json",
        ])
        .output()
        .unwrap();
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.contains("Keep this body"));
    assert!(!json.contains("Must not overwrite"));

    proxy
        .call::<_, _, ()>("CloseNotification", &(id,))
        .await
        .unwrap();
    let signal = tokio::time::timeout(std::time::Duration::from_secs(1), closed_signals.next())
        .await
        .unwrap()
        .unwrap();
    let (closed_id, reason): (u32, u32) = signal.body().deserialize().unwrap();
    assert_eq!((closed_id, reason), (id, 3));

    daemon.kill().ok();
    daemon.wait().ok();
}

#[tokio::test]
async fn dev_dbus_close_notification_emits_notification_closed_signal() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = test_bus_name("CloseSignal");

    let mut daemon = support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
        "--database",
        support::path_arg(&database),
        "--dev-bus-name",
        bus_name.as_str(),
    ]);

    support::wait_for_status_with_child(&socket, &mut daemon);

    let connection = Connection::session().await.unwrap();
    let proxy = Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let mut closed_signals = proxy.receive_signal("NotificationClosed").await.unwrap();

    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Calendar",
                0u32,
                "",
                "Meeting",
                "Starting soon",
                Vec::<String>::new(),
                HashMap::<String, OwnedValue>::new(),
                -1i32,
            ),
        )
        .await
        .unwrap();

    proxy
        .call::<_, _, ()>("CloseNotification", &(id))
        .await
        .unwrap();

    let signal = tokio::time::timeout(std::time::Duration::from_secs(1), closed_signals.next())
        .await
        .unwrap()
        .unwrap();
    let (closed_id, reason): (u32, u32) = signal.body().deserialize().unwrap();
    assert_eq!(closed_id, id);
    assert_eq!(reason, 3);

    daemon.kill().ok();
    daemon.wait().ok();
}

#[tokio::test]
async fn dev_dbus_action_invocation_signal_precedes_dismissal_close_signal() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = test_bus_name("ActionSignal");

    let mut daemon = support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
        "--database",
        support::path_arg(&database),
        "--dev-bus-name",
        bus_name.as_str(),
    ]);

    support::wait_for_status_with_child(&socket, &mut daemon);

    let connection = Connection::session().await.unwrap();
    let proxy = Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let mut action_signals = proxy.receive_signal("ActionInvoked").await.unwrap();
    let mut closed_signals = proxy.receive_signal("NotificationClosed").await.unwrap();

    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Calendar",
                0u32,
                "",
                "Meeting",
                "Starting soon",
                vec!["open".to_string(), "Open".to_string()],
                HashMap::<String, OwnedValue>::new(),
                -1i32,
            ),
        )
        .await
        .unwrap();

    let response = control_socket_round_trip(&socket, &format!("invoke-action {id} open\n"));
    assert_eq!(response, "ok\n");

    let action_signal =
        tokio::time::timeout(std::time::Duration::from_secs(1), action_signals.next())
            .await
            .unwrap()
            .unwrap();
    let (action_id, action_key): (u32, String) = action_signal.body().deserialize().unwrap();
    assert_eq!(action_id, id);
    assert_eq!(action_key, "open");

    let closed_signal =
        tokio::time::timeout(std::time::Duration::from_secs(1), closed_signals.next())
            .await
            .unwrap()
            .unwrap();
    let (closed_id, reason): (u32, u32) = closed_signal.body().deserialize().unwrap();
    assert_eq!(closed_id, id);
    assert_eq!(reason, 2);

    daemon.kill().ok();
    daemon.wait().ok();
}

#[tokio::test]
async fn dev_dbus_close_missing_notification_returns_empty_dbus_error() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = test_bus_name("MissingClose");

    let mut daemon = support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
        "--database",
        support::path_arg(&database),
        "--dev-bus-name",
        bus_name.as_str(),
    ]);

    support::wait_for_status_with_child(&socket, &mut daemon);

    let connection = Connection::session().await.unwrap();
    let proxy = Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();

    let error = proxy
        .call::<_, _, ()>("CloseNotification", &(12345u32))
        .await
        .unwrap_err();
    let message = error.to_string();
    assert!(message.contains("org.freedesktop.DBus.Error.Failed"));
    assert!(!message.contains("missing"));
    assert!(!message.contains("notification 12345"));

    daemon.kill().ok();
    daemon.wait().ok();
}

fn test_bus_name(suffix: &str) -> String {
    format!("org.canopa.NotifyTest{}{}", std::process::id(), suffix)
}

fn control_socket_round_trip(socket: &std::path::Path, request: &str) -> String {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}
