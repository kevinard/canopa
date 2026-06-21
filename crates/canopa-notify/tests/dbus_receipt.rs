mod support;

use assert_cmd::Command;
use futures_util::StreamExt;
use std::collections::HashMap;
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

    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Example Mail",
                0u32,
                "",
                "Inbox",
                "One unread message",
                Vec::<String>::new(),
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
    assert!(
        capabilities
            .iter()
            .any(|capability| capability == "actions")
    );
    assert!(capabilities.iter().any(|capability| capability == "body"));
    assert!(
        capabilities
            .iter()
            .any(|capability| capability == "icon-static")
    );
    assert!(
        !capabilities
            .iter()
            .any(|capability| capability == "body-images")
    );
    assert!(
        !capabilities
            .iter()
            .any(|capability| capability == "icon-multi")
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
