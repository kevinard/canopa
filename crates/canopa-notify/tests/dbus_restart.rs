mod support;

use canopa_notify::{
    ControlReply, NotificationCenterProjection, NotificationStatus, NotificationThread,
};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Child;
use std::time::Duration;
use zbus::zvariant::OwnedValue;

// Terminate both fixture lifetimes even if an assertion fails.
struct DaemonProcess(Child);

impl DaemonProcess {
    fn start(socket: &Path, bus_name: &str) -> Self {
        let mut process = Self(support::spawn_canopa_notify([
            "daemon",
            "--headless",
            "--socket",
            support::path_arg(socket),
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

async fn notify(proxy: &zbus::Proxy<'_>, replaces_id: u32, summary: &str, transient: bool) -> u32 {
    proxy
        .call(
            "Notify",
            &(
                "Restart test",
                replaces_id,
                "",
                summary,
                "",
                Vec::<String>::new(),
                HashMap::from([("transient".to_string(), OwnedValue::from(transient))]),
                0i32,
            ),
        )
        .await
        .unwrap()
}

fn control_request(socket: &Path, request: &str) -> String {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn center(socket: &Path) -> NotificationCenterProjection {
    let reply = control_request(
        socket,
        "control {\"command\":\"notification-center-projection\"}\n",
    );
    let ControlReply::NotificationCenterProjection(projection) =
        serde_json::from_str(&reply).unwrap()
    else {
        panic!("unexpected control reply: {reply}");
    };
    projection
}

async fn owner_change(changes: &mut zbus::proxy::SignalStream<'_>) -> (String, String, String) {
    tokio::time::timeout(Duration::from_secs(3), changes.next())
        .await
        .expect("missing service-owner transition")
        .unwrap()
        .body()
        .deserialize()
        .unwrap()
}

#[tokio::test]
async fn restart_changes_service_owner_without_registering_retained_history_as_live() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("control.sock");
    let bus_name = format!("org.canopa.NotifyRestartTest{}", std::process::id());
    let connection = zbus::Connection::session().await.unwrap();
    let bus = zbus::Proxy::new(
        &connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )
    .await
    .unwrap();
    let mut changes = bus
        .receive_signal_with_args("NameOwnerChanged", &[(0, bus_name.as_str())])
        .await
        .unwrap();

    let first = DaemonProcess::start(&socket, &bus_name);
    let (name, previous, first_owner) = owner_change(&mut changes).await;
    assert_eq!(name, bus_name);
    assert!(previous.is_empty());
    let first_proxy = zbus::Proxy::new(
        &connection,
        first_owner.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    assert_eq!(notify(&first_proxy, 0, "", false).await, 1);
    assert_eq!(
        notify(&first_proxy, 0, "Retained before restart", false).await,
        2
    );
    assert_eq!(
        notify(&first_proxy, 0, "Transient before restart", true).await,
        3
    );
    let original = center(&socket).groups.remove(0).items.remove(0);
    assert_eq!(
        original.notification_id.get(),
        1,
        "fixture must separate wire and retained IDs"
    );
    assert_eq!(original.status, NotificationStatus::Active);

    drop(first);
    let (_, old_owner, new_owner) = owner_change(&mut changes).await;
    assert_eq!(old_owner, first_owner);
    assert!(new_owner.is_empty());

    let _second = DaemonProcess::start(&socket, &bus_name);
    let (_, previous, second_owner) = owner_change(&mut changes).await;
    assert!(previous.is_empty());
    assert_ne!(second_owner, first_owner);
    assert!(!second_owner.is_empty());
    let proxy = zbus::Proxy::new(
        &connection,
        second_owner.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    // A client that pins requests to the old unique owner cannot address the successor.
    assert!(
        first_proxy
            .call::<_, _, ()>("CloseNotification", &(2u32,))
            .await
            .is_err()
    );
    for id in [1u32, 2, 3] {
        assert!(
            proxy
                .call::<_, _, ()>("CloseNotification", &(id,))
                .await
                .is_err()
        );
    }
    let stale = center(&socket);
    assert_eq!(stale.groups.len(), 1);
    assert_eq!(stale.groups[0].items.len(), 1);
    assert_eq!(
        stale.groups[0].items[0].notification_id,
        original.notification_id
    );
    assert_eq!(stale.groups[0].items[0].status, NotificationStatus::Stale);
    assert_eq!(stale.groups[0].items[0].summary, "Retained before restart");

    // Numeric values can recur across owners, but never bind old history into the new registry.
    assert_eq!(notify(&proxy, 0, "Current", false).await, 1);
    assert_eq!(notify(&proxy, 2, "New fixed token", false).await, 2);
    assert_eq!(
        notify(&proxy, 2, "Live fixed-token replacement", false).await,
        2
    );
    let active: Vec<NotificationThread> =
        serde_json::from_str(&control_request(&socket, "active-json\n")).unwrap();
    assert_eq!(active.len(), 2);
    assert!(
        active
            .iter()
            .all(|thread| thread.id != original.notification_id)
    );
    let fixed = active
        .iter()
        .find(|thread| thread.accepted_notification.summary == "Live fixed-token replacement")
        .unwrap();
    assert_eq!(fixed.revision, 2);
    let mut closures = proxy.receive_signal("NotificationClosed").await.unwrap();
    proxy
        .call::<_, _, ()>("CloseNotification", &(2u32,))
        .await
        .unwrap();
    let signal = tokio::time::timeout(Duration::from_secs(3), closures.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(signal.header().sender().unwrap().as_str(), second_owner);
    assert_eq!(signal.body().deserialize::<(u32, u32)>().unwrap(), (2, 3));
    let active: Vec<NotificationThread> =
        serde_json::from_str(&control_request(&socket, "active-json\n")).unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].accepted_notification.summary, "Current");
    let after = center(&socket);
    let retained = after
        .groups
        .iter()
        .flat_map(|group| &group.items)
        .find(|item| item.notification_id == original.notification_id)
        .unwrap();
    assert_eq!(retained, &stale.groups[0].items[0]);
    assert!(
        after
            .groups
            .iter()
            .flat_map(|group| &group.items)
            .all(|item| item.summary != "Transient before restart")
    );
}
