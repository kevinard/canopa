use crate::support;

use canopa_notify::{Daemon, DaemonOptions, DeliveryOutcome};
use support::notifications::notification;

struct Process(std::process::Child);

impl Drop for Process {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

#[tokio::test]
async fn failed_presentation_installation_preserves_configuration_without_exposing_adapter_details()
{
    struct RejectPrivateTheme;
    impl canopa_notify::SoundPresenter for RejectPrivateTheme {
        fn apply_sound_settings(
            &self,
            settings: canopa_notify::NotificationSoundSettings,
        ) -> anyhow::Result<()> {
            anyhow::ensure!(settings.theme != "private", "private adapter detail");
            Ok(())
        }
        fn play_notification_sound(
            &self,
            _: canopa_notify::NotificationSoundPlayback,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        fn stop_all_sounds(&self) {}
        fn supports_sound(&self) -> bool {
            false
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let good = temp.path().join("good.kdl");
    let bad = temp.path().join("bad.kdl");
    std::fs::write(&good, "rule app=\"Mail\" outcome=\"silenced\"\n").unwrap();
    std::fs::write(
        &bad,
        "sound theme=\"private\"\nrule app=\"Mail\" outcome=\"drop\"\n",
    )
    .unwrap();
    let options = |path| {
        DaemonOptions::graphical(
            temp.path().join("history.db"),
            canopa_notify::NoopBannerPresenter,
        )
        .with_sound_presenter(RejectPrivateTheme)
        .with_configuration_file(path)
    };
    let daemon = Daemon::open(options(good.clone())).await.unwrap();
    let error = daemon.reload_configuration_file(&bad).await.unwrap_err();
    assert!(!format!("{error:#}").contains("private"));
    let status = daemon.configuration_status();
    assert_eq!(status.generation, 1);
    assert_eq!(status.path.as_ref(), Some(&good));
    assert!(!status.error.unwrap().contains("private"));
    assert_eq!(
        daemon
            .receive_notification(notification("Mail", "Old rules survive"))
            .await
            .unwrap()
            .delivery_outcome,
        DeliveryOutcome::Silenced
    );
    drop(daemon);
    let Err(error) = Daemon::open(options(bad)).await else {
        panic!("invalid startup sound installation succeeded");
    };
    assert!(!format!("{error:#}").contains("private"));
}

fn cli(socket: &std::path::Path, arguments: &[&str]) -> std::process::Output {
    crate::support::cargo_bin()
        .unwrap()
        .args(["ctl", "--socket", support::path_arg(socket)])
        .args(arguments)
        .timeout(std::time::Duration::from_secs(5))
        .output()
        .unwrap()
}

async fn occupy_blocking_worker() -> (std::sync::mpsc::Sender<()>, tokio::task::JoinHandle<()>) {
    // Dropping release also frees the worker if an assertion unwinds.
    let (release, held) = std::sync::mpsc::channel::<()>();
    let (started, ready) = tokio::sync::oneshot::channel();
    let worker = tokio::task::spawn_blocking(move || {
        started.send(()).unwrap();
        let _ = held.recv();
    });
    ready.await.unwrap();
    (release, worker)
}

#[test]
fn invalid_reload_targets_leave_the_executable_available_and_recoverable() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("good.kdl");
    let fifo = temp.path().join("fifo.kdl");
    let socket = temp.path().join("control.sock");
    std::fs::write(&config, "sound enabled=#false\n").unwrap();
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        0,
    )
    .unwrap();
    let mut daemon = Process(support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
        "--config",
        support::path_arg(&config),
    ]));
    support::wait_for_status_with_child(&socket, &mut daemon.0);
    for path in [
        fifo,
        temp.path().to_path_buf(),
        temp.path().join("missing.kdl"),
    ] {
        let output = cli(&socket, &["reload", support::path_arg(&path)]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let output = cli(&socket, &["status", "--json"]);
        assert!(output.status.success());
        let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(status["configuration"]["generation"], 1);
        assert_eq!(status["configuration"]["path"], support::path_arg(&config));
        assert!(status["configuration"]["error"].is_string());
    }
    assert!(cli(&socket, &["reload"]).status.success());
    let output = cli(&socket, &["status", "--json"]);
    let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["configuration"]["generation"], 2);
    assert!(status["configuration"]["error"].is_null());
}

#[test]
fn queued_configuration_preparation_keeps_current_policy_and_controls_available() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
            .await
            .unwrap();
        let path = temp.path().join("next.kdl");
        std::fs::write(&path, "rule app=\"Mail\" outcome=\"drop\"\n").unwrap();
        let (release, worker) = occupy_blocking_worker().await;
        let entered = std::sync::Arc::new(tokio::sync::Notify::new());
        let reload = tokio::spawn({
            let daemon = daemon.clone();
            let entered = entered.clone();
            async move {
                entered.notify_one();
                daemon.reload_configuration_file(&path).await
            }
        });
        entered.notified().await;
        assert_eq!(
            daemon.configuration_status().generation,
            0,
            "file preparation ran on the async executor"
        );
        assert!(daemon.change_do_not_disturb(Some(true)).await.unwrap());
        assert_eq!(
            daemon
                .receive_notification(notification("Mail", "While queued"))
                .await
                .unwrap()
                .delivery_outcome,
            DeliveryOutcome::Silenced
        );
        release.send(()).unwrap();
        worker.await.unwrap();
        reload.await.unwrap().unwrap();
        assert_eq!(daemon.configuration_status().generation, 1);
        daemon.change_do_not_disturb(Some(false)).await.unwrap();
        assert_eq!(
            daemon
                .receive_notification(notification("Mail", "After preparation"))
                .await
                .unwrap()
                .delivery_outcome,
            DeliveryOutcome::Dropped
        );
    });
}

#[test]
fn cancelled_configuration_preparation_cannot_publish_late_or_block_a_later_reload() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
            .await
            .unwrap();
        let cancelled_path = temp.path().join("cancelled.kdl");
        let next_path = temp.path().join("next.kdl");
        std::fs::write(&cancelled_path, "rule app=\"Mail\" outcome=\"drop\"\n").unwrap();
        std::fs::write(&next_path, "rule app=\"Mail\" outcome=\"silenced\"\n").unwrap();
        let (release, worker) = occupy_blocking_worker().await;
        let entered = std::sync::Arc::new(tokio::sync::Notify::new());
        let cancelled = tokio::spawn({
            let daemon = daemon.clone();
            let entered = entered.clone();
            async move {
                entered.notify_one();
                daemon.reload_configuration_file(&cancelled_path).await
            }
        });
        entered.notified().await;
        cancelled.abort();
        assert!(cancelled.await.unwrap_err().is_cancelled());
        let next = tokio::spawn({
            let daemon = daemon.clone();
            let path = next_path.clone();
            async move { daemon.reload_configuration_file(&path).await }
        });
        assert_eq!(daemon.configuration_status().generation, 0);
        release.send(()).unwrap();
        worker.await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), next)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let status = daemon.configuration_status();
        assert_eq!(status.generation, 1);
        assert_eq!(status.path.as_ref(), Some(&next_path));
        assert!(status.error.is_none());
        assert_eq!(
            daemon
                .receive_notification(notification("Mail", "Only latest applies"))
                .await
                .unwrap()
                .delivery_outcome,
            DeliveryOutcome::Silenced
        );
    });
}

#[tokio::test]
async fn concurrent_configuration_reloads_commit_complete_generations_and_preserve_user_policy() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let first = temp.path().join("silenced.kdl");
    let second = temp.path().join("dropped.kdl");
    std::fs::write(&first, "rule app=\"Mail\" outcome=\"silenced\"\n").unwrap();
    std::fs::write(&second, "rule app=\"Mail\" outcome=\"drop\"\n").unwrap();
    daemon
        .upsert_user_rule(canopa_notify::UserRule::for_app(
            "Chat",
            DeliveryOutcome::Dropped,
        ))
        .await
        .unwrap();
    let mut jobs = tokio::task::JoinSet::new();
    for index in 0..16 {
        let daemon = daemon.clone();
        let path = if index % 2 == 0 {
            first.clone()
        } else {
            second.clone()
        };
        jobs.spawn(async move { daemon.reload_configuration_file(&path).await });
    }
    while let Some(result) = jobs.join_next().await {
        result.unwrap().unwrap();
    }
    let status = daemon.configuration_status();
    assert_eq!(status.generation, 16);
    assert!(status.error.is_none());
    let expected = if status.path.as_ref() == Some(&first) {
        DeliveryOutcome::Silenced
    } else {
        assert_eq!(status.path.as_ref(), Some(&second));
        DeliveryOutcome::Dropped
    };
    assert_eq!(
        daemon
            .receive_notification(notification("Mail", "Final generation"))
            .await
            .unwrap()
            .delivery_outcome,
        expected
    );
    assert_eq!(
        daemon
            .receive_notification(notification("Chat", "User rule survives"))
            .await
            .unwrap()
            .delivery_outcome,
        DeliveryOutcome::Dropped
    );
}

#[tokio::test]
async fn installed_configuration_and_failed_cli_reload_preserve_dbus_policy() {
    let temp = tempfile::tempdir().unwrap();
    let config_root = temp.path().join("config");
    let config = config_root.join("canopa-notify/config.kdl");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "rule app=\"Mail\" outcome=\"silenced\"\n").unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let bus_name = format!("org.canopa.NotifyTest.ConfigP{}", std::process::id());
    let mut daemon = Process(
        support::canopa_notify_command()
            .env("XDG_CONFIG_HOME", &config_root)
            .args([
                "daemon",
                "--headless",
                "--socket",
                support::path_arg(&socket),
                "--database",
                support::path_arg(&database),
                "--dev-bus-name",
                &bus_name,
            ])
            .spawn()
            .unwrap(),
    );
    support::wait_for_status_with_child(&socket, &mut daemon.0);
    let connection = zbus::Connection::session().await.unwrap();
    let proxy = zbus::Proxy::new(
        &connection,
        bus_name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let notify = async |summary: &str| {
        proxy
            .call::<_, _, u32>(
                "Notify",
                &(
                    "Mail",
                    0u32,
                    "",
                    summary,
                    "",
                    Vec::<String>::new(),
                    std::collections::HashMap::<String, zbus::zvariant::OwnedValue>::new(),
                    0i32,
                ),
            )
            .await
            .unwrap()
    };
    assert!(notify("Before reload").await > 0);
    let invalid = temp.path().join("bad.kdl");
    std::fs::write(&invalid, "rule app=\"Mail\" outcome=\"private-invalid\"\n").unwrap();
    let rejected = cli(&socket, &["reload", support::path_arg(&invalid)]);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&rejected.stderr).contains("private"));
    let output = cli(&socket, &["status", "--json"]);
    assert!(output.status.success());
    let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["configuration"]["generation"], 1);
    assert_eq!(status["configuration"]["path"], support::path_arg(&config));
    assert!(
        !status["configuration"]["error"]
            .as_str()
            .unwrap()
            .contains("private")
    );
    assert!(notify("After failed reload").await > 0);
    std::fs::write(&config, "rule app=\"Mail\" outcome=\"shown\"\n").unwrap();
    assert!(cli(&socket, &["reload"]).status.success());
    assert!(notify("After valid reload").await > 0);
    let output = cli(&socket, &["active", "--json"]);
    assert!(output.status.success());
    let active: Vec<canopa_notify::NotificationThread> =
        serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(active.len(), 3);
    for thread in active {
        let expected = if thread.accepted_notification.summary == "After valid reload" {
            DeliveryOutcome::Shown
        } else {
            DeliveryOutcome::Silenced
        };
        assert_eq!(thread.delivery_outcome, expected);
    }
}

#[tokio::test]
async fn configuration_file_admission_preserves_committed_state_and_accepts_regular_symlinks() {
    let temp = tempfile::tempdir().unwrap();
    let good = temp.path().join("good.kdl");
    let link = temp.path().join("current.kdl");
    let invalid = temp.path().join("invalid.kdl");
    let prefix = "rule app=\"Mail\" outcome=\"silenced\"\n//";
    let exact = format!("{prefix}{}\n", "x".repeat(1_048_576 - prefix.len() - 1));
    std::fs::write(&good, exact.as_bytes()).unwrap();
    std::os::unix::fs::symlink(&good, &link).unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    daemon.reload_configuration_file(&link).await.unwrap();
    let committed = daemon.configuration_status();
    assert_eq!(committed.generation, 1);
    assert_eq!(committed.path.as_ref(), Some(&link));
    for bytes in [(exact + " ").into_bytes(), vec![0xff, 0xfe]] {
        std::fs::write(&invalid, bytes).unwrap();
        assert!(daemon.reload_configuration_file(&invalid).await.is_err());
        let status = daemon.configuration_status();
        assert_eq!(status.generation, committed.generation);
        assert_eq!(status.path, committed.path);
        assert!(status.error.is_some());
        assert_eq!(
            daemon
                .receive_notification(notification("Mail", "Preserved policy"))
                .await
                .unwrap()
                .delivery_outcome,
            DeliveryOutcome::Silenced
        );
    }
    assert!(daemon.reload_configuration_file(temp.path()).await.is_err());
    assert!(
        daemon
            .reload_configuration_file(&temp.path().join("missing.kdl"))
            .await
            .is_err()
    );
    std::fs::write(&good, "rule app=\"Mail\" outcome=\"drop\"\n").unwrap();
    daemon.reload_configuration_file(&link).await.unwrap();
    let status = daemon.configuration_status();
    assert_eq!(status.generation, 2);
    assert_eq!(status.path, committed.path);
    assert!(status.error.is_none());
    assert_eq!(
        daemon
            .receive_notification(notification("Mail", "Updated policy"))
            .await
            .unwrap()
            .delivery_outcome,
        DeliveryOutcome::Dropped
    );
}

#[tokio::test]
async fn configuration_errors_do_not_retain_arbitrary_names_values_or_source() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("bad.kdl");
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    for source in [
        "private-node\n",
        "rule app=\"Mail\" outcome=\"drop\" private-property=1\n",
        "sound private-property=1\n",
        "rule app=\"Mail\" outcome=\"private-outcome\"\n",
        "rule app=\"Mail\" outcome=\"drop\" urgency=\"private-urgency\"\n",
        "rule app=\"private-value\" outcome=\n",
    ] {
        std::fs::write(&path, source).unwrap();
        let error = daemon.reload_configuration_file(&path).await.unwrap_err();
        assert!(
            !format!("{error:#}").contains("private"),
            "configuration content leaked in error chain"
        );
        assert!(
            !format!("{error:?}").contains("private"),
            "configuration source retained by diagnostic"
        );
        let status = daemon.configuration_status();
        assert_eq!(status.generation, 0);
        assert!(!status.error.unwrap().contains("private"));
    }
}

#[test]
fn fifo_configuration_fails_promptly_without_waiting_for_a_writer() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("notify.kdl");
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &path,
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        0,
    )
    .unwrap();
    let output = crate::support::cargo_bin()
        .unwrap()
        .args([
            "daemon",
            "--headless",
            "--config",
            support::path_arg(&path),
            "--socket",
            support::path_arg(&temp.path().join("control.sock")),
            "--database",
            support::path_arg(&temp.path().join("history.db")),
        ])
        .timeout(std::time::Duration::from_secs(2))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("regular UTF-8 file"),
        "configuration did not reject the FIFO: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn unsupported_type_annotations_cannot_commit_configuration() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = Daemon::open(DaemonOptions::headless(temp.path().join("history.db")))
        .await
        .unwrap();
    let path = temp.path().join("typed.kdl");
    for source in [
        "(private)rule app=\"Mail\" outcome=\"drop\"\n",
        "rule (private)\"id\" app=\"Mail\" outcome=\"drop\"\n",
        "rule app=(private)\"Mail\" outcome=\"drop\"\n",
        "(private)sound enabled=#false\n",
        "sound enabled=(private)#false\n",
        "sound theme=(private)\"freedesktop\"\n",
        "(private)icons scale=2\n",
        "icons scale=(private)2\n",
        "icons theme=(private)\"hicolor\"\n",
    ] {
        std::fs::write(&path, source).unwrap();
        assert!(
            daemon.reload_configuration_file(&path).await.is_err(),
            "undefined type annotation was accepted: {source}"
        );
        let status = daemon.configuration_status();
        assert_eq!(status.generation, 0);
        assert_eq!(status.path, None);
        assert!(!status.error.unwrap().contains("private"));
    }
    assert_eq!(
        daemon
            .receive_notification(notification("Mail", "Still allowed"))
            .await
            .unwrap()
            .delivery_outcome,
        DeliveryOutcome::Shown
    );
}

#[tokio::test]
async fn unsupported_sound_children_reject_the_whole_reload() {
    let temp = tempfile::tempdir().unwrap();
    let good = temp.path().join("good.kdl");
    let bad = temp.path().join("bad.kdl");
    std::fs::write(&good, "rule app=\"Mail\" outcome=\"silenced\"\n").unwrap();
    std::fs::write(
        &bad,
        "sound { enabled #false; }\nrule app=\"Mail\" outcome=\"drop\"\n",
    )
    .unwrap();
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db"))
            .with_configuration_file(good.clone()),
    )
    .await
    .unwrap();
    assert!(
        daemon.reload_configuration_file(&bad).await.is_err(),
        "unsupported child settings were silently accepted"
    );
    let status = daemon.configuration_status();
    assert_eq!(status.generation, 1);
    assert_eq!(status.path.as_ref(), Some(&good));
    assert!(status.error.is_some());
    let receipt = daemon
        .receive_notification(notification("Mail", "Still silenced"))
        .await
        .unwrap();
    assert_eq!(receipt.delivery_outcome, DeliveryOutcome::Silenced);
}

#[tokio::test]
async fn common_reload_preserves_last_committed_source_and_isolates_shell_schema() {
    use canopa_config::{Format, Request};
    use canopa_notify::{ControlCommand, NotifyControl};
    let temp = tempfile::tempdir().unwrap();
    let common = temp.path().join("common.kdl");
    let legacy = temp.path().join("legacy.kdl");
    std::fs::write(&common, "schema-version 1\nshell { invalid-shell-option 42; }\nnotifications { rule app=\"Mail\" outcome=\"silenced\"; }\n").unwrap();
    std::fs::write(&legacy, "rule app=\"Mail\" outcome=\"drop\"\n").unwrap();
    let daemon = Daemon::open(
        DaemonOptions::headless(temp.path().join("history.db"))
            .with_configuration_source(Request::common(&common)),
    )
    .await
    .unwrap();
    let original = daemon.configuration_status();
    assert_eq!(
        original.source.as_ref().unwrap().format,
        Some(Format::CanopaV1)
    );
    assert_eq!(
        daemon
            .receive_notification(notification("Mail", "first"))
            .await
            .unwrap()
            .delivery_outcome,
        DeliveryOutcome::Silenced
    );
    let control = NotifyControl::new(daemon.clone()).with_configuration_path(legacy.clone());
    std::fs::write(
        &common,
        "schema-version 1\nnotifications { private-value 42; }\n",
    )
    .unwrap();
    assert!(
        control
            .execute(ControlCommand::ReloadConfiguration { path: None })
            .await
            .is_err()
    );
    let failed = daemon.configuration_status();
    assert_eq!(failed.generation, original.generation);
    assert_eq!(failed.source, original.source);
    assert!(!failed.error.unwrap().contains("private-value"));
    assert_eq!(
        daemon
            .receive_notification(notification("Mail", "still old"))
            .await
            .unwrap()
            .delivery_outcome,
        DeliveryOutcome::Silenced
    );
    // An absent namespace selects feature defaults, not any legacy source.
    std::fs::write(&common, "schema-version 1\nshell {}\n").unwrap();
    control
        .execute(ControlCommand::ReloadConfiguration { path: None })
        .await
        .unwrap();
    assert!(daemon.configuration_status().source.unwrap().defaults);
    assert_eq!(
        daemon
            .receive_notification(notification("Mail", "defaults"))
            .await
            .unwrap()
            .delivery_outcome,
        DeliveryOutcome::Shown
    );
    // An explicit legacy reload becomes the committed request for subsequent no-path reloads.
    control
        .execute(ControlCommand::ReloadConfiguration {
            path: Some(legacy.clone()),
        })
        .await
        .unwrap();
    std::fs::write(&legacy, "rule app=\"Mail\" outcome=\"silenced\"\n").unwrap();
    control
        .execute(ControlCommand::ReloadConfiguration { path: None })
        .await
        .unwrap();
    assert_eq!(daemon.configuration_status().path, Some(legacy));
    assert_eq!(
        daemon
            .receive_notification(notification("Mail", "legacy"))
            .await
            .unwrap()
            .delivery_outcome,
        DeliveryOutcome::Silenced
    );
    daemon.shutdown().await.unwrap();
}

#[test]
fn discovery_reload_adopts_common_source_and_explicit_common_reload_stays_selected() {
    let temp = tempfile::tempdir().unwrap();
    let config_root = temp.path().join("config");
    let legacy = config_root.join("canopa-notify/config.kdl");
    let common = config_root.join("canopa/config.kdl");
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    std::fs::create_dir_all(common.parent().unwrap()).unwrap();
    std::fs::write(&legacy, "sound enabled=#false\n").unwrap();
    let socket = temp.path().join("control.sock");
    let database = temp.path().join("history.db");
    let mut daemon = Process(
        support::canopa_notify_command()
            .env("XDG_CONFIG_HOME", &config_root)
            .args([
                "daemon",
                "--headless",
                "--socket",
                support::path_arg(&socket),
                "--database",
                support::path_arg(&database),
            ])
            .spawn()
            .unwrap(),
    );
    support::wait_for_status_with_child(&socket, &mut daemon.0);
    let status = || {
        let output = cli(&socket, &["status", "--json"]);
        assert!(output.status.success());
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["configuration"]
            .clone()
    };
    assert_eq!(status()["source"]["format"], "legacy");
    std::fs::write(
        &common,
        "schema-version 1\nnotifications { sound enabled=#true; }\n",
    )
    .unwrap();
    assert!(cli(&socket, &["reload"]).status.success());
    assert_eq!(status()["path"], support::path_arg(&common));
    assert_eq!(status()["source"]["format"], "canopa-v1");
    let explicit = temp.path().join("explicit.kdl");
    std::fs::write(&explicit, "schema-version 1\nnotifications {}\n").unwrap();
    assert!(
        cli(&socket, &["reload-canopa", support::path_arg(&explicit)])
            .status
            .success()
    );
    std::fs::write(&common, "invalid shared source\n").unwrap();
    assert!(cli(&socket, &["reload"]).status.success());
    assert_eq!(status()["path"], support::path_arg(&explicit));
}
