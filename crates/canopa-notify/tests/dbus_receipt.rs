mod support;

use assert_cmd::Command;
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

    let (name, vendor, _version, _spec_version): (String, String, String, String) =
        proxy.call("GetServerInformation", &()).await.unwrap();
    assert_eq!(name, "Canopa Notify");
    assert_eq!(vendor, "Canopa");

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
    assert!(json.contains("5%"));
    assert!(!json.contains("0%"));

    daemon.kill().ok();
    daemon.wait().ok();
}

fn test_bus_name(suffix: &str) -> String {
    format!("org.canopa.NotifyTest{}{}", std::process::id(), suffix)
}
