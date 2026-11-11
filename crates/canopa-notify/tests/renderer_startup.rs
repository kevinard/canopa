#![cfg(feature = "gui")]

use canopa_notify::{Daemon, DaemonOptions, NotificationInput};
use std::os::unix::net::UnixListener;
use std::time::Duration;

#[tokio::test]
async fn unresponsive_wayland_startup_preserves_history_and_does_not_publish_legacy_import() {
    // Both cases share this private test bus sequentially; neither contacts the real compositor.
    for legacy_import in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let socket = temp.path().join("control.sock");
        let destination = temp.path().join("state/canopa-notify/history.db");
        let database = if legacy_import {
            socket.with_extension("db")
        } else {
            destination.clone()
        };
        let seed = Daemon::open(DaemonOptions::headless(database))
            .await
            .unwrap();
        seed.receive_notification(NotificationInput {
            app_name: "Startup test".into(),
            summary: "Preserve live history".into(),
            ..Default::default()
        })
        .await
        .unwrap();
        let before = seed.snapshot().await.unwrap();

        // Connections succeed, but no Wayland registry/configure/redraw response is supplied.
        let display = temp.path().join("unresponsive-wayland");
        let _compositor = UnixListener::bind(&display).unwrap();
        let mut command = assert_cmd::cargo::cargo_bin_cmd!("canopa-notify");
        command
            .env("CANOPA_LANGUAGE", "en")
            .env("WAYLAND_DISPLAY", display)
            .env_remove("WAYLAND_SOCKET")
            .env("XDG_RUNTIME_DIR", temp.path())
            .env("XDG_STATE_HOME", temp.path().join("state"))
            .env("XDG_CONFIG_HOME", temp.path().join("config"))
            .args(["daemon", "--service", "--socket"])
            .arg(&socket)
            .timeout(Duration::from_secs(15))
            .assert()
            .code(1)
            .stderr(predicates::str::contains(
                "banner renderer readiness timed out",
            ));

        assert_eq!(seed.snapshot().await.unwrap(), before);
        assert_eq!(destination.exists(), !legacy_import);
        assert!(
            !socket.exists(),
            "failed startup retained its control endpoint"
        );
    }
}
