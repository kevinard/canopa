use crate::support;

use canopa_notify::{
    AppPermissionControl, ControlCommand, ControlReply, Daemon, DaemonOptions, DeliveryOutcome,
    NotificationAction, NotificationInput, NotificationStatus, NotificationThread, UserRule,
};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

struct Fixture {
    temp: tempfile::TempDir,
    socket: PathBuf,
    source: PathBuf,
    destination: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let socket = temp.path().join("control.sock");
        let source = socket.with_extension("db");
        let destination = temp.path().join("state/canopa-notify/history.db");
        Self {
            temp,
            socket,
            source,
            destination,
        }
    }

    fn command(&self, socket: &Path, database: Option<&Path>) -> Command {
        // Do not use the spawn helper: its explicit --database disables import.
        let mut command = support::canopa_notify_command();
        command
            .env("XDG_STATE_HOME", self.temp.path().join("state"))
            .env("XDG_CONFIG_HOME", self.temp.path().join("config"))
            .args([
                "daemon",
                "--headless",
                "--socket",
                support::path_arg(socket),
            ]);
        if let Some(database) = database {
            command.args(["--database", support::path_arg(database)]);
        }
        command
    }
}

struct Process(Child);

impl Process {
    fn start(mut command: Command, socket: &Path) -> Self {
        let mut process = Self(command.spawn().unwrap());
        support::wait_for_status_with_child(socket, &mut process.0);
        process
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

fn control(socket: &Path, command: ControlCommand) -> ControlReply {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    writeln!(
        stream,
        "control {}",
        serde_json::to_string(&command).unwrap()
    )
    .unwrap();
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).unwrap();
    serde_json::from_str(&reply).unwrap()
}

async fn seed_history(path: &Path, summary: &str) -> NotificationThread {
    let daemon = Daemon::open(DaemonOptions::headless(path.to_path_buf()))
        .await
        .unwrap();
    daemon
        .receive_notification(NotificationInput {
            app_name: "Import test".into(),
            summary: summary.into(),
            body: "<b>Keep</b> the body".into(),
            category: Some("email.arrived".into()),
            actions: vec![NotificationAction::new("open", "Open")],
            ..Default::default()
        })
        .await
        .unwrap();
    daemon.set_do_not_disturb(true).await.unwrap();
    daemon
        .upsert_user_rule(UserRule::for_app("Import test", DeliveryOutcome::Silenced))
        .await
        .unwrap();
    daemon
        .upsert_user_rule(
            UserRule::for_app_category("Import test", "email.arrived", DeliveryOutcome::Dropped)
                .unwrap(),
        )
        .await
        .unwrap();
    daemon
        .snapshot()
        .await
        .unwrap()
        .history
        .into_iter()
        .next()
        .unwrap()
}

fn assert_imported(socket: &Path, original: &NotificationThread) {
    assert_eq!(
        control(socket, ControlCommand::DoNotDisturb),
        ControlReply::DoNotDisturb(true)
    );
    assert_eq!(
        control(socket, ControlCommand::ActiveNotifications),
        ControlReply::ActiveNotifications(vec![])
    );
    let ControlReply::NotificationCenterProjection(center) =
        control(socket, ControlCommand::NotificationCenterProjection)
    else {
        panic!("expected a center projection");
    };
    assert_eq!(center.groups.len(), 1);
    let group = &center.groups[0];
    assert_eq!(group.permission, AppPermissionControl::Silence);
    assert_eq!(group.category_permissions.len(), 1);
    assert_eq!(group.category_permissions[0].category, "email.arrived");
    assert_eq!(
        group.category_permissions[0].permission,
        AppPermissionControl::Drop
    );
    assert_eq!(group.items.len(), 1);
    let item = &group.items[0];
    assert_eq!(item.notification_id, original.id);
    assert_eq!(item.status, NotificationStatus::Stale);
    assert_eq!(item.summary, original.accepted_notification.summary);
    assert_eq!(item.body, original.accepted_notification.body);
    assert_eq!(
        item.body_content,
        original.accepted_notification.body_content
    );
    assert!(
        item.actions.is_empty(),
        "imported history must not regain live actions"
    );
}

#[tokio::test]
async fn legacy_import_rejects_a_source_owned_through_another_socket() {
    let fixture = Fixture::new();
    let owner_socket = fixture.temp.path().join("owner.sock");
    let name = format!("org.canopa.LegacyOwner{}", std::process::id());
    let mut command = fixture.command(&owner_socket, Some(&fixture.source));
    command.args(["--dev-bus-name", &name]);
    let _owner = Process::start(command, &owner_socket);
    let connection = zbus::connection::Builder::session()
        .unwrap()
        .method_timeout(Duration::from_secs(3))
        .build()
        .await
        .unwrap();
    let proxy = zbus::Proxy::new(
        &connection,
        name.as_str(),
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .unwrap();
    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Import test",
                0u32,
                "",
                "Keep live",
                "",
                vec!["open", "Open"],
                HashMap::<String, zbus::zvariant::OwnedValue>::new(),
                0i32,
            ),
        )
        .await
        .unwrap();
    let before = control(&owner_socket, ControlCommand::ActiveNotifications);

    assert_cmd::Command::from_std(fixture.command(&fixture.socket, None))
        .timeout(Duration::from_secs(3))
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "legacy notification history is in use",
        ));

    assert!(!fixture.destination.exists(), "a live source was imported");
    assert!(
        !fixture.socket.exists(),
        "failed startup retained its endpoint"
    );
    assert_eq!(
        control(&owner_socket, ControlCommand::ActiveNotifications),
        before
    );
    proxy
        .call::<_, _, ()>("CloseNotification", &(id,))
        .await
        .unwrap();
    assert_eq!(
        control(&owner_socket, ControlCommand::ActiveNotifications),
        ControlReply::ActiveNotifications(vec![])
    );
}

#[tokio::test]
async fn stopped_legacy_history_import_includes_committed_wal_and_preserves_source() {
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new();
    // Keep a SQLite connection open so closing the seed does not checkpoint the
    // WAL. It is a fixture peer, not a running Notification Daemon.
    let wal_keeper = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&fixture.source)
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal)
                .pragma("wal_autocheckpoint", "0"),
        )
        .await
        .unwrap();
    let main_before = std::fs::read(&fixture.source).unwrap();
    let original = seed_history(&fixture.source, "Committed in WAL").await;
    let wal_path = fixture.source.with_extension("db-wal");
    let wal_before = std::fs::read(&wal_path).unwrap();
    assert!(wal_before.len() > 32, "fixture has no committed WAL frames");
    assert_eq!(
        std::fs::read(&fixture.source).unwrap(),
        main_before,
        "fixture unexpectedly checkpointed"
    );

    let imported = Process::start(fixture.command(&fixture.socket, None), &fixture.socket);
    assert_imported(&fixture.socket, &original);
    assert_eq!(std::fs::read(&fixture.source).unwrap(), main_before);
    assert_eq!(std::fs::read(&wal_path).unwrap(), wal_before);
    for path in [&fixture.destination, &fixture.socket] {
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    drop(imported);
    // The installed destination is reused, not imported again on every startup.
    let _restarted = Process::start(fixture.command(&fixture.socket, None), &fixture.socket);
    assert_imported(&fixture.socket, &original);
    wal_keeper.close().await;
}

#[tokio::test]
async fn failed_import_preserves_source_and_allows_a_repaired_retry() {
    let fixture = Fixture::new();
    let invalid = b"Not a SQLite database; preserve this source";
    std::fs::write(&fixture.source, invalid).unwrap();
    assert_cmd::Command::from_std(fixture.command(&fixture.socket, None))
        .timeout(Duration::from_secs(3))
        .assert()
        .code(1);
    assert_eq!(std::fs::read(&fixture.source).unwrap(), invalid);
    assert!(!fixture.destination.exists());
    assert!(!fixture.socket.exists());
    let remaining: Vec<_> = std::fs::read_dir(fixture.destination.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(
        remaining,
        vec![std::ffi::OsString::from("history.db.lock")],
        "failed import left notification content behind"
    );

    // Model a user repairing the source, preserving the original for recovery.
    let preserved = fixture.temp.path().join("invalid-source.backup");
    std::fs::rename(&fixture.source, &preserved).unwrap();
    let original = seed_history(&fixture.source, "Repaired source").await;
    let _daemon = Process::start(fixture.command(&fixture.socket, None), &fixture.socket);
    assert_imported(&fixture.socket, &original);
    assert_eq!(std::fs::read(preserved).unwrap(), invalid);
}

#[tokio::test]
async fn existing_destination_is_reused_even_while_legacy_source_is_owned() {
    let fixture = Fixture::new();
    seed_history(&fixture.source, "Old location").await;
    let destination = seed_history(&fixture.destination, "Already migrated").await;
    let owner_socket = fixture.temp.path().join("owner.sock");
    let _owner = Process::start(
        fixture.command(&owner_socket, Some(&fixture.source)),
        &owner_socket,
    );
    let source_before = std::fs::read(&fixture.source).unwrap();
    let owner_before = control(&owner_socket, ControlCommand::NotificationCenterProjection);

    let _daemon = Process::start(fixture.command(&fixture.socket, None), &fixture.socket);
    assert_imported(&fixture.socket, &destination);
    assert_eq!(std::fs::read(&fixture.source).unwrap(), source_before);
    assert_eq!(
        control(&owner_socket, ControlCommand::NotificationCenterProjection),
        owner_before
    );
}

#[tokio::test]
async fn interrupted_import_preserves_source_and_retries_without_partial_copies() {
    use std::os::unix::process::ExitStatusExt;

    let fixture = Fixture::new();
    let original = seed_history(&fixture.source, "Survives interrupted copy").await;
    // Make the real VACUUM copy exceed the child's file-size limit. SIGXFSZ
    // terminates it during a write without running Rust cleanup/destructors.
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&fixture.source))
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE import_padding (bytes BLOB); INSERT INTO import_padding VALUES (zeroblob(1048576));")
        .execute(&pool).await.unwrap();
    pool.close().await;
    let source_before = std::fs::read(&fixture.source).unwrap();
    let normal = fixture.command(&fixture.socket, None);
    let mut limited = Command::new("prlimit");
    limited
        .args(["--fsize=131072:131072", "--core=0:0", "--"])
        .arg(normal.get_program())
        .args(normal.get_args());
    for (key, value) in normal.get_envs() {
        if let Some(value) = value {
            limited.env(key, value);
        }
    }
    let output = assert_cmd::Command::from_std(limited)
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();
    assert_eq!(output.status.signal(), Some(25), "{output:?}");
    assert!(
        !fixture.destination.exists(),
        "partial import became authoritative"
    );
    assert_eq!(std::fs::read(&fixture.source).unwrap(), source_before);
    let partials: Vec<_> = std::fs::read_dir(fixture.destination.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with('.') && !name.ends_with("-journal")
        })
        .collect();
    assert_eq!(
        partials.len(),
        1,
        "fixture must interrupt an actual temporary copy"
    );
    assert!(std::fs::metadata(&partials[0]).unwrap().len() > 0);

    let _daemon = Process::start(fixture.command(&fixture.socket, None), &fixture.socket);
    assert_imported(&fixture.socket, &original);
    assert_eq!(std::fs::read(&fixture.source).unwrap(), source_before);
    assert!(
        !partials[0].exists(),
        "retry retained private notification content in an orphan copy"
    );
    assert!(
        !std::fs::read_dir(fixture.destination.parent().unwrap())
            .unwrap()
            .any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".history-import-")),
        "retry retained an interrupted SQLite sidecar"
    );
}

#[tokio::test]
async fn published_destination_reclaims_only_reserved_import_files_without_a_source() {
    let fixture = Fixture::new();
    let original = seed_history(&fixture.destination, "Published history").await;
    let parent = fixture.destination.parent().unwrap();
    let orphan = parent.join(".history-import-interrupted");
    let sidecar = parent.join(".history-import-interrupted-journal");
    let unrelated = parent.join(".user-backup");
    for path in [&orphan, &sidecar, &unrelated] {
        std::fs::write(path, b"preserve unless owned by import").unwrap();
    }
    let _daemon = Process::start(fixture.command(&fixture.socket, None), &fixture.socket);
    assert_imported(&fixture.socket, &original);
    assert!(!orphan.exists());
    assert!(!sidecar.exists());
    assert_eq!(
        std::fs::read(unrelated).unwrap(),
        b"preserve unless owned by import"
    );
}

#[tokio::test]
async fn import_cleanup_rejects_symlinks_without_removing_their_target() {
    let fixture = Fixture::new();
    seed_history(&fixture.destination, "Published history").await;
    let target = fixture.temp.path().join("keep");
    std::fs::write(&target, b"keep this content").unwrap();
    let link = fixture
        .destination
        .parent()
        .unwrap()
        .join(".history-import-link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert_cmd::Command::from_std(fixture.command(&fixture.socket, None))
        .timeout(Duration::from_secs(3))
        .assert()
        .code(1)
        .stderr(predicates::str::contains("owned regular file"));
    assert!(link.is_symlink());
    assert_eq!(std::fs::read(target).unwrap(), b"keep this content");
}
