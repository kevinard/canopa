use crate::support;

use canopa_notify::{ControlReply, NotificationStatus};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Child;
use std::time::{Duration, Instant};
use zbus::zvariant::OwnedValue;

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

fn control(socket: &Path, command: &str) -> String {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(command.as_bytes()).unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    reply
}

fn terminate(process: &mut Process) {
    assert!(
        std::process::Command::new("kill")
            .args(["-TERM", &process.0.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = process.0.try_wait().unwrap() {
            assert!(status.success(), "SIGTERM did not exit cleanly: {status}");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "SIGTERM failed to stop the daemon"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[tokio::test]
async fn sigterm_drains_closures_releases_ownership_and_restores_pending_attention() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("notify.sock");
    let name = format!("org.canopa.NotifyShutdown{}", std::process::id());
    let start = || {
        let mut process = Process(support::spawn_canopa_notify([
            "daemon",
            "--headless",
            "--socket",
            support::path_arg(&socket),
            "--dev-bus-name",
            &name,
        ]));
        support::wait_for_status_with_child(&socket, &mut process.0);
        process
    };
    let mut first = start();
    let connection = zbus::Connection::session().await.unwrap();
    let proxy = zbus::Proxy::new(
        &connection,
        name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let mut closed = zbus::MessageStream::for_match_rule(
        zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender(name.as_str())
            .unwrap()
            .interface("org.freedesktop.Notifications")
            .unwrap()
            .member("NotificationClosed")
            .unwrap()
            .build(),
        &connection,
        Some(256),
    )
    .await
    .unwrap();
    let mut ids = Vec::new();
    for _ in 0..160 {
        ids.push(
            proxy
                .call::<_, _, u32>(
                    "Notify",
                    &(
                        "Shutdown",
                        0u32,
                        "",
                        "Close before exit",
                        "",
                        Vec::<String>::new(),
                        HashMap::<String, OwnedValue>::new(),
                        0i32,
                    ),
                )
                .await
                .unwrap(),
        );
    }
    assert_eq!(
        control(
            &socket,
            "control {\"command\":\"clear-active-notifications\"}\n"
        )
        .trim(),
        "{\"reply\":\"command-applied\"}"
    );
    // Keep one deliberate attention item across the stop, independently of the drained closures.
    let _: u32 = proxy
        .call(
            "Notify",
            &(
                "Shutdown",
                0u32,
                "",
                "Pending after restart",
                "",
                Vec::<String>::new(),
                HashMap::<String, OwnedValue>::new(),
                0i32,
            ),
        )
        .await
        .unwrap();
    assert!(
        control(
            &socket,
            "control {\"command\":\"change-do-not-disturb\",\"enabled\":true}\n"
        )
        .contains("true")
    );
    terminate(&mut first);
    assert!(!socket.exists(), "shutdown retained its private socket");
    // Consume only after process exit, exercising the real publisher and bus buffers.
    let received = tokio::time::timeout(Duration::from_secs(5), async {
        let mut received = Vec::new();
        for _ in 0..ids.len() {
            let (id, reason): (u32, u32) = closed
                .next()
                .await
                .unwrap()
                .unwrap()
                .body()
                .deserialize()
                .unwrap();
            assert_eq!(reason, 2);
            received.push(id);
        }
        received
    })
    .await
    .unwrap();
    // Clearing follows the current newest-first notification order.
    assert_eq!(received, ids.into_iter().rev().collect::<Vec<_>>());
    let mut second = start();
    let reply = control(
        &socket,
        "control {\"command\":\"notification-center-projection\"}\n",
    );
    let ControlReply::NotificationCenterProjection(center) = serde_json::from_str(&reply).unwrap()
    else {
        panic!("unexpected Center reply")
    };
    let pending: Vec<_> = center
        .groups
        .iter()
        .flat_map(|group| &group.items)
        .filter(|item| item.requires_attention)
        .collect();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].summary, "Pending after restart");
    assert_eq!(pending[0].status, NotificationStatus::Stale);
    assert!(pending[0].actions.is_empty());
    assert!(
        control(&socket, "control {\"command\":\"status\"}\n").contains("\"do_not_disturb\":true")
    );
    terminate(&mut second);
}
