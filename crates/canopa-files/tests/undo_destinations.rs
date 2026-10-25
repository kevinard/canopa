use canopa_files::{
    Cancellation,
    operations::{OperationEngine, UndoChoice},
    transfers::{ConflictChoice, TransferKind, TransferReply, TransferRequest},
};
use std::fs;

struct PreservedFixture(Option<tempfile::TempDir>);
impl PreservedFixture {
    fn new(prefix: &str) -> Self {
        Self(Some(
            tempfile::Builder::new().prefix(prefix).tempdir().unwrap(),
        ))
    }
    fn path(&self) -> &std::path::Path {
        self.0.as_ref().unwrap().path()
    }
}
impl Drop for PreservedFixture {
    fn drop(&mut self) {
        if std::thread::panicking()
            && let Some(root) = self.0.take()
        {
            eprintln!(
                "retained expanded undo_destinations fixture: {}",
                root.keep().display()
            );
        }
    }
}

#[test]
fn restarting_a_killed_keep_both_destination_restore_preserves_newer_files_and_cleans_stages() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    let root = PreservedFixture::new("canopa-destination-undo-crash-");
    let status = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "crash_destination_restore_helper"])
        .env("CANOPA_FILES_DESTINATION_CRASH_ROOT", root.path())
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    assert!(!root.path().join("undo-returned").exists());
    let resumed_at = std::time::Instant::now();
    let engine = OperationEngine::open(root.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    engine.undo(&cancel).unwrap();
    assert!(
        resumed_at.elapsed() < std::time::Duration::from_secs(20),
        "resuming and reversing 500 Keep both restorations took {:?}; batch Undo must stay responsive",
        resumed_at.elapsed()
    );
    let destination = root.path().join("destination");
    for index in 0..500 {
        assert_eq!(
            fs::read(destination.join(format!("file-{index:04}"))).unwrap(),
            b"newer work after copying"
        );
        assert!(!destination.join(format!("file-{index:04} (2)")).exists());
    }
    assert!(
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(
        fs::read_dir(&destination).unwrap().count(),
        500,
        "restored backups must leave no abandoned staging folders"
    );
}

#[test]
#[ignore = "child process fixture; killed after an actual previous-destination restoration"]
fn crash_destination_restore_helper() {
    let root = std::path::PathBuf::from(
        std::env::var_os("CANOPA_FILES_DESTINATION_CRASH_ROOT").expect("parent fixture root"),
    );
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-destination-undo-crash-")
    );
    let older = root.join("older");
    let incoming = root.join("incoming");
    let destination = root.join("destination");
    for directory in [&older, &incoming, &destination] {
        fs::create_dir(directory).unwrap();
    }
    for index in 0..500 {
        fs::write(
            older.join(format!("file-{index:04}")),
            b"previous destination",
        )
        .unwrap();
        fs::write(
            incoming.join(format!("file-{index:04}")),
            b"incoming replacement",
        )
        .unwrap();
    }
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let cancel = Cancellation::default();
    for source in [older, incoming] {
        let outcome = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: (0..500)
                        .map(|index| source.join(format!("file-{index:04}")))
                        .collect(),
                    destination: destination.clone(),
                },
                &cancel,
                |_| TransferReply::Resolve {
                    choice: ConflictChoice::Replace,
                    apply_to_remaining: true,
                },
            )
            .unwrap();
        assert!(outcome.failures.is_empty());
    }
    for index in 0..500 {
        fs::write(
            destination.join(format!("file-{index:04}")),
            b"newer work after copying",
        )
        .unwrap();
    }
    let restored = destination.join("file-0499 (2)");
    let ready = std::sync::Arc::new(std::sync::Barrier::new(2));
    let watcher_ready = ready.clone();
    std::thread::spawn(move || {
        watcher_ready.wait();
        while !restored.exists() {
            std::thread::sleep(std::time::Duration::from_micros(50));
        }
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    });
    ready.wait();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    fs::write(root.join("undo-returned"), b"missed crash window").unwrap();
    panic!("the child was not killed during destination restoration");
}

fn copy(engine: &OperationEngine, source: std::path::PathBuf, destination: std::path::PathBuf) {
    let outcome = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
                destination,
            },
            &Cancellation::default(),
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(outcome.failures.is_empty());
}

#[test]
fn keeping_both_when_undoing_a_changed_replacement_restores_the_previous_file_and_older_history() {
    let root = tempfile::tempdir().unwrap();
    let older = root.path().join("older");
    let incoming = root.path().join("incoming");
    let destination = root.path().join("destination");
    for directory in [&older, &incoming, &destination] {
        fs::create_dir(directory).unwrap();
    }
    fs::write(older.join("report.txt"), b"previous destination").unwrap();
    fs::write(incoming.join("report.txt"), b"incoming replacement").unwrap();
    let state = root.path().join("state/history");
    let engine = OperationEngine::open(&state).unwrap();
    let cancel = Cancellation::default();
    copy(&engine, older.join("report.txt"), destination.clone());
    copy(&engine, incoming.join("report.txt"), destination.clone());
    fs::write(
        destination.join("report.txt"),
        b"newer work after the replacement",
    )
    .unwrap();
    let outcome = engine
        .undo_interactive(None, &cancel, |conflict| {
            assert_eq!(
                conflict.existing.as_ref().unwrap().path,
                destination.join("report.txt")
            );
            assert_eq!(conflict.incoming.entry().unwrap().size, 20);
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(outcome.completed, 1);
    assert_eq!(outcome.remaining, 0);
    assert_eq!(
        fs::read(destination.join("report.txt")).unwrap(),
        b"newer work after the replacement"
    );
    let returned = destination.join("report (2).txt");
    assert_eq!(fs::read(&returned).unwrap(), b"previous destination");
    drop(engine);
    let engine = OperationEngine::open(&state).unwrap();
    engine.undo(&cancel).unwrap();
    assert!(!returned.exists());
    assert_eq!(
        fs::read(destination.join("report.txt")).unwrap(),
        b"newer work after the replacement"
    );
    assert!(
        engine
            .recovery_items(None, &cancel)
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn replacing_when_undoing_a_changed_copy_restores_the_previous_destination_and_protects_newer_work()
{
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("report.txt"), b"incoming copy").unwrap();
    fs::write(destination.join("report.txt"), b"previous destination").unwrap();
    let state = root.path().join("state/history");
    let engine = OperationEngine::open(&state).unwrap();
    let cancel = Cancellation::default();
    copy(&engine, source.join("report.txt"), destination.clone());
    fs::remove_file(destination.join("report.txt")).unwrap();
    fs::write(
        destination.join("report.txt"),
        b"a newer file replacing the copy",
    )
    .unwrap();
    let outcome = engine
        .undo_interactive(None, &cancel, |conflict| {
            assert_eq!(
                conflict.source,
                canopa_files::operations::UndoSource::PreviousDestination
            );
            UndoChoice::Replace
        })
        .unwrap();
    assert_eq!(outcome.completed, 1);
    assert_eq!(
        fs::read(destination.join("report.txt")).unwrap(),
        b"previous destination"
    );
    drop(engine);
    let engine = OperationEngine::open(&state).unwrap();
    let items = engine.recovery_items(None, &cancel).unwrap().items;
    assert_eq!(items.len(), 1);
    assert!(!items[0].held_for_undo);
    let recovered = root.path().join("Recovered newer work");
    engine.recover_copy(&items[0], &recovered, &cancel).unwrap();
    assert_eq!(
        fs::read(recovered).unwrap(),
        b"a newer file replacing the copy"
    );
}

#[test]
fn skipped_previous_destinations_remain_retryable_across_cancellation_and_restart() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    for name in ["a.txt", "b.txt"] {
        fs::write(source.join(name), b"incoming copy").unwrap();
        fs::write(destination.join(name), b"previous destination").unwrap();
    }
    let state = root.path().join("state/history");
    let engine = OperationEngine::open(&state).unwrap();
    let cancel = Cancellation::default();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.join("a.txt"), source.join("b.txt")],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: true,
            },
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    for name in ["a.txt", "b.txt"] {
        fs::write(destination.join(name), b"newer work").unwrap();
    }
    let outcome = engine
        .undo_interactive(copied.id, &cancel, |conflict| {
            if conflict.existing.as_ref().unwrap().path.ends_with("b.txt") {
                UndoChoice::Skip
            } else {
                UndoChoice::KeepBoth
            }
        })
        .unwrap();
    assert_eq!(outcome.completed, 1);
    assert_eq!(outcome.remaining, 1);
    assert_eq!(outcome.skipped, vec![destination.join("b.txt")]);
    assert_eq!(
        fs::read(destination.join("a (2).txt")).unwrap(),
        b"previous destination"
    );
    assert_eq!(fs::read(destination.join("b.txt")).unwrap(), b"newer work");
    drop(engine);
    let engine = OperationEngine::open(&state).unwrap();
    let cancelled = engine
        .undo_interactive(copied.id, &cancel, |_| UndoChoice::Cancel)
        .unwrap();
    assert!(cancelled.cancelled);
    assert_eq!(cancelled.completed, 0);
    assert_eq!(cancelled.remaining, 1);
    assert_eq!(engine.recovery_items(None, &cancel).unwrap().items.len(), 1);
    let completed = engine
        .undo_interactive(copied.id, &cancel, |_| UndoChoice::Replace)
        .unwrap();
    assert_eq!(completed.completed, 1);
    assert_eq!(completed.remaining, 0);
    assert_eq!(
        fs::read(destination.join("b.txt")).unwrap(),
        b"previous destination"
    );
    assert_eq!(fs::read(destination.join("a.txt")).unwrap(), b"newer work");
    let protected = engine.recovery_items(None, &cancel).unwrap().items;
    assert_eq!(protected.len(), 1);
    assert!(!protected[0].held_for_undo);
    let recovered = root.path().join("Recovered b.txt");
    engine
        .recover_copy(&protected[0], &recovered, &cancel)
        .unwrap();
    assert_eq!(fs::read(recovered).unwrap(), b"newer work");
}

#[test]
fn keep_both_relocates_older_namespace_history_through_intermediate_regular_file_versions() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    let target = root.path().join("target");
    let first = root.path().join("first");
    let second = root.path().join("second");
    for directory in [&destination, &target, &first, &second] {
        fs::create_dir(directory).unwrap();
    }
    let alias = destination.join("alias");
    symlink("../target", &alias).unwrap();
    let payload = root.path().join("payload.txt");
    fs::write(&payload, b"older work through the directory link").unwrap();
    fs::write(first.join("alias"), b"first regular replacement").unwrap();
    fs::write(second.join("alias"), b"second regular replacement").unwrap();
    let state = root.path().join("state/history");
    let engine = OperationEngine::open(&state).unwrap();
    let cancel = Cancellation::default();
    copy(&engine, payload, alias.clone());
    // A broken directory link is still a replaceable link, with a namespace
    // that becomes traversable again once that older version is restored.
    let away = root.path().join("target temporarily away");
    fs::rename(&target, &away).unwrap();
    copy(&engine, first.join("alias"), destination.clone());
    fs::rename(&away, &target).unwrap();
    copy(&engine, second.join("alias"), destination.clone());
    fs::write(&alias, b"newest work stays at the original name").unwrap();
    engine
        .undo_interactive(None, &cancel, |_| UndoChoice::KeepBoth)
        .unwrap();
    assert_eq!(
        fs::read(destination.join("alias (2)")).unwrap(),
        b"first regular replacement"
    );
    drop(engine);
    let engine = OperationEngine::open(&state).unwrap();
    engine.undo(&cancel).unwrap();
    assert_eq!(
        fs::read_link(destination.join("alias (2)")).unwrap(),
        std::path::Path::new("../target")
    );
    engine.undo(&cancel).unwrap();
    assert!(!target.join("payload.txt").exists());
    assert_eq!(
        fs::read(alias).unwrap(),
        b"newest work stays at the original name"
    );
}
