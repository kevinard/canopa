use canopa_files::{
    Cancellation,
    operations::{OperationEngine, UndoChoice},
    transfers::{TransferKind, TransferReply, TransferRequest},
};
use std::{fs, os::unix::fs::MetadataExt, path::PathBuf};

struct Fixture {
    _source: tempfile::TempDir,
    _destination: tempfile::TempDir,
    history: PathBuf,
    engine: OperationEngine,
    original: PathBuf,
    current: PathBuf,
}

fn other_filesystem(source: &std::path::Path) -> tempfile::TempDir {
    let device = fs::metadata(source).unwrap().dev();
    [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|path| {
            (fs::metadata(&path).ok()?.dev() != device)
                .then(|| {
                    tempfile::Builder::new()
                        .prefix(".canopa-directory-move-")
                        .tempdir_in(path)
                        .ok()
                })
                .flatten()
        })
        .expect("two writable filesystems")
}

#[test]
fn undo_returns_only_the_moved_tree_from_a_preexisting_destination_and_preserves_its_other_files() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let source = tempfile::tempdir().unwrap();
    let destination = other_filesystem(source.path());
    let original = source.path().join("project");
    let current = destination.path().join("project");
    fs::create_dir_all(original.join("nested/deep")).unwrap();
    fs::create_dir(original.join("empty")).unwrap();
    fs::write(original.join("nested/deep/note.txt"), b"moved nested work").unwrap();
    symlink("deep/note.txt", original.join("nested/link")).unwrap();
    fs::set_permissions(&original, fs::Permissions::from_mode(0o750)).unwrap();
    fs::create_dir_all(current.join("nested")).unwrap();
    fs::write(current.join("existing.txt"), b"existing root work").unwrap();
    fs::write(current.join("nested/existing.txt"), b"existing nested work").unwrap();
    let root_identity = fs::metadata(&current).unwrap().ino();
    let nested_identity = fs::metadata(current.join("nested")).unwrap().ino();
    let history = source.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![original.clone()],
                destination: destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Resolve {
                choice: canopa_files::transfers::ConflictChoice::Merge,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(result.failures.is_empty());
    assert!(!original.exists());
    drop(engine);
    let engine = OperationEngine::open(history).unwrap();
    let returned = engine
        .undo_interactive(None, &cancel, |_| {
            panic!("preexisting destination data must stay in place without an Undo conflict")
        })
        .unwrap();
    assert_eq!(returned.remaining, 0);
    assert_eq!(
        fs::read(original.join("nested/deep/note.txt")).unwrap(),
        b"moved nested work"
    );
    assert_eq!(
        fs::read_link(original.join("nested/link")).unwrap(),
        PathBuf::from("deep/note.txt")
    );
    assert!(original.join("empty").is_dir());
    assert_eq!(
        fs::metadata(&original).unwrap().permissions().mode() & 0o777,
        0o750
    );
    assert!(!original.join("existing.txt").exists());
    assert!(!original.join("nested/existing.txt").exists());
    assert_eq!(fs::metadata(&current).unwrap().ino(), root_identity);
    assert_eq!(
        fs::metadata(current.join("nested")).unwrap().ino(),
        nested_identity
    );
    assert_eq!(
        fs::read(current.join("existing.txt")).unwrap(),
        b"existing root work"
    );
    assert_eq!(
        fs::read(current.join("nested/existing.txt")).unwrap(),
        b"existing nested work"
    );
    assert!(!current.join("nested/deep").exists());
    assert!(!current.join("nested/link").exists());
    assert!(!current.join("empty").exists());
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
}
impl Fixture {
    fn copied_tree() -> Self {
        Self::copied_tree_with(0)
    }

    fn copied_tree_with(extra: usize) -> Self {
        let source = tempfile::Builder::new()
            .prefix("canopa-directory-move-")
            .tempdir()
            .unwrap();
        let device = fs::metadata(source.path()).unwrap().dev();
        let destination = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
            .into_iter()
            .find_map(|path| {
                (fs::metadata(&path).ok()?.dev() != device)
                    .then(|| {
                        tempfile::Builder::new()
                            .prefix(".canopa-directory-move-")
                            .tempdir_in(path)
                            .ok()
                    })
                    .flatten()
            })
            .unwrap();
        let seed = source.path().join("seed/project");
        fs::create_dir_all(&seed).unwrap();
        fs::write(seed.join("a.txt"), b"older copied a").unwrap();
        fs::write(seed.join("z.txt"), b"older copied z").unwrap();
        for index in 0..extra {
            fs::write(
                seed.join(format!("item-{index:04}.txt")),
                format!("owned work {index}"),
            )
            .unwrap();
        }
        let parent = source.path().join("original");
        fs::create_dir(&parent).unwrap();
        let original = parent.join("project");
        let history = source.path().join("state/history");
        let engine = OperationEngine::open(&history).unwrap();
        let cancel = Cancellation::default();
        let copied = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![seed],
                    destination: parent,
                },
                &cancel,
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert!(copied.failures.is_empty());
        let moved = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![original.clone()],
                    destination: destination.path().to_owned(),
                },
                &cancel,
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert!(moved.failures.is_empty());
        let current = destination.path().join("project");
        Self {
            _source: source,
            _destination: destination,
            history,
            engine,
            original,
            current,
        }
    }
}

#[test]
fn a_killed_owned_directory_return_resumes_without_duplicates_and_retains_older_copy_ownership() {
    use std::{os::unix::process::ExitStatusExt, process::Command};
    let fixture = Fixture::copied_tree_with(128);
    fs::create_dir(&fixture.original).unwrap();
    fs::write(fixture.original.join("newer.txt"), b"newer work").unwrap();
    let returned = fixture.original.with_file_name("project (2)");
    drop(fixture.engine);
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "crash_moved_directory_destination_helper",
        ])
        .env("CANOPA_DIRECTORY_MOVE_ROOT", fixture._source.path())
        .env("CANOPA_DIRECTORY_MOVE_CURRENT", &fixture.current)
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    assert!(returned.is_dir());
    let unreturned = fs::read_to_string(fixture._source.path().join("witness"))
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert!(unreturned > 0);
    let engine = OperationEngine::open(&fixture.history).unwrap();
    let cancel = Cancellation::default();
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
    let result = engine
        .undo_interactive(None, &cancel, |_| {
            panic!("the owned destination name and recorded children must survive reopening")
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert!(!fixture.current.exists());
    for index in 0..128 {
        assert_eq!(
            fs::read(returned.join(format!("item-{index:04}.txt"))).unwrap(),
            format!("owned work {index}").as_bytes()
        );
        assert!(!returned.join(format!("item-{index:04} (2).txt")).exists());
    }
    assert_eq!(fs::read(returned.join("a.txt")).unwrap(), b"older copied a");
    assert_eq!(fs::read(returned.join("z.txt")).unwrap(), b"older copied z");
    engine
        .undo_interactive(None, &cancel, |_| {
            panic!("older Copy owns the returned directory")
        })
        .unwrap();
    assert!(!returned.exists());
    assert_eq!(
        fs::read(fixture.original.join("newer.txt")).unwrap(),
        b"newer work"
    );
    assert_eq!(
        fs::read(fixture._source.path().join("seed/project/item-0127.txt")).unwrap(),
        b"owned work 127"
    );
}

#[test]
#[ignore = "child fixture; killed after the actual owned return directory appears"]
fn crash_moved_directory_destination_helper() {
    use std::time::{Duration, Instant};
    let root =
        PathBuf::from(std::env::var_os("CANOPA_DIRECTORY_MOVE_ROOT").expect("parent fixture root"));
    assert!(
        root.starts_with(std::env::temp_dir())
            && root
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"canopa-directory-move-")
    );
    let current = PathBuf::from(
        std::env::var_os("CANOPA_DIRECTORY_MOVE_CURRENT").expect("parent fixture current"),
    );
    assert!(
        current
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b".canopa-directory-move-")
    );
    let original = root.join("original/project");
    let returned = original.with_file_name("project (2)");
    let witness = root.join("witness");
    let watcher = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !returned.is_dir() {
            assert!(
                Instant::now() < deadline,
                "the return directory never appeared"
            );
            std::thread::sleep(Duration::from_micros(50));
        }
        let unreturned = fs::read_dir(current).unwrap().count();
        fs::write(witness, unreturned.to_string()).unwrap();
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
    });
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    engine
        .undo_interactive(None, &Cancellation::default(), |question| {
            assert_eq!(question.destination, original);
            UndoChoice::KeepBoth
        })
        .unwrap();
    watcher.join().unwrap();
}

#[test]
fn undo_recreates_owned_original_directories_without_a_conflict_and_preserves_older_copy_undo() {
    let fixture = Fixture::copied_tree();
    let cancel = Cancellation::default();
    fixture.engine.undo(&cancel).unwrap();
    assert!(!fixture.current.exists());
    assert_eq!(
        fs::read(fixture.original.join("a.txt")).unwrap(),
        b"older copied a"
    );
    fixture.engine.undo(&cancel).unwrap();
    assert!(!fixture.original.exists());
}

#[test]
fn retrying_a_failed_directory_move_finishes_its_owned_empty_source_folders_and_can_undo_the_retry()
{
    let source = tempfile::tempdir().unwrap();
    let destination = other_filesystem(source.path());
    let original = source.path().join("project");
    fs::create_dir_all(original.join("nested")).unwrap();
    let failed = original.join("nested/blocked");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &failed,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    fs::write(original.join("z.txt"), b"already moved work").unwrap();
    let current = destination.path().join("project");
    let engine = OperationEngine::open(source.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![original.clone()],
                destination: destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(moved.failures.len(), 1);
    assert_eq!(moved.failures[0].source, failed);
    assert_eq!(
        fs::read(current.join("z.txt")).unwrap(),
        b"already moved work"
    );
    fs::remove_file(&failed).unwrap();
    fs::write(&failed, b"repaired source work").unwrap();
    let retry = moved
        .retry_continuation
        .expect("failed directory work must retain its exact parent cleanup plan");
    let retried = engine
        .resume_transfer(&retry, &cancel, |_| TransferReply::Continue)
        .unwrap();
    assert!(retried.failures.is_empty());
    assert_eq!(
        fs::read(current.join("nested/blocked")).unwrap(),
        b"repaired source work"
    );
    assert!(
        !original.exists(),
        "Retry must finish the original owned directory cleanup after its last failed leaf succeeds"
    );
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&failed).unwrap(), b"repaired source work");
    assert_eq!(
        fs::read(current.join("z.txt")).unwrap(),
        b"already moved work"
    );
    engine.undo(&cancel).unwrap();
    assert!(!current.exists());
    assert_eq!(
        fs::read(original.join("z.txt")).unwrap(),
        b"already moved work"
    );
}

#[test]
fn undo_of_a_resumed_directory_move_keeps_both_names_and_preserves_the_earlier_move_and_copy_history()
 {
    use canopa_files::{operations::OperationRequest, transfers::TransferEvent};
    let source = tempfile::tempdir().unwrap();
    let destination = other_filesystem(source.path());
    let seed = source.path().join("seed/project");
    fs::create_dir_all(seed.join("one")).unwrap();
    fs::create_dir_all(seed.join("two")).unwrap();
    fs::write(seed.join("one/a.txt"), b"first moved work").unwrap();
    fs::write(seed.join("two/b.txt"), b"later moved work").unwrap();
    fs::write(seed.join("z.txt"), b"root moved work").unwrap();
    let parent = source.path().join("original");
    fs::create_dir(&parent).unwrap();
    let original = parent.join("project");
    let current = destination.path().join("project");
    let history = source.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![seed.clone()],
                destination: parent.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    let first = original.join("one/a.txt");
    let paused = engine.transfer(&TransferRequest { kind: TransferKind::Move, sources: vec![original.clone()], destination: destination.path().to_owned() }, &Cancellation::default(), |event| {
        if matches!(event, TransferEvent::Progress { ref path, .. } if path == &first && !first.exists()) {
            TransferReply::Cancel
        } else { TransferReply::Continue }
    }).unwrap();
    assert!(paused.cancelled);
    assert!(paused.failures.is_empty());
    assert_eq!(
        fs::read(current.join("one/a.txt")).unwrap(),
        b"first moved work"
    );
    assert!(original.join("one").is_dir());
    assert!(original.join("two/b.txt").is_file());
    let side_before = source.path().join("unrelated-before");
    let side_after = source.path().join("unrelated-after");
    fs::write(&side_before, b"intervening independent work").unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: side_before.clone(),
                destination: side_after.clone(),
            },
            &cancel,
        )
        .unwrap();
    let resumed = engine
        .resume_transfer(&paused.continuation.unwrap(), &cancel, |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(resumed.failures.is_empty() && !resumed.cancelled && !resumed.needs_attention);
    assert!(!original.exists());
    fs::create_dir(&original).unwrap();
    fs::write(original.join("newer.txt"), b"newer original work").unwrap();
    let newer_identity = fs::metadata(&original).unwrap().ino();
    let returned = parent.join("project (2)");
    let mut questions = 0;
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            questions += 1;
            assert_eq!(question.destination, original);
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(questions, 1);
    assert_eq!(result.remaining, 0);
    assert!(returned.join("one").is_dir());
    assert!(!returned.join("one/a.txt").exists());
    assert_eq!(
        fs::read(current.join("one/a.txt")).unwrap(),
        b"first moved work"
    );
    assert_eq!(
        fs::read(returned.join("two/b.txt")).unwrap(),
        b"later moved work"
    );
    assert_eq!(
        fs::read(returned.join("z.txt")).unwrap(),
        b"root moved work"
    );
    engine.undo(&cancel).unwrap();
    assert_eq!(
        fs::read(&side_before).unwrap(),
        b"intervening independent work"
    );
    assert!(!side_after.exists());
    drop(engine);
    let engine = OperationEngine::open(history).unwrap();
    engine
        .undo_interactive(None, &cancel, |_| {
            panic!("the earlier moved child must follow the kept-both root")
        })
        .unwrap();
    assert!(!current.exists());
    assert_eq!(
        fs::read(returned.join("one/a.txt")).unwrap(),
        b"first moved work"
    );
    engine
        .undo_interactive(None, &cancel, |_| {
            panic!("the earlier Copy must own all returned directory identities")
        })
        .unwrap();
    assert!(!returned.exists());
    assert_eq!(fs::metadata(&original).unwrap().ino(), newer_identity);
    assert_eq!(
        fs::read(original.join("newer.txt")).unwrap(),
        b"newer original work"
    );
    assert_eq!(
        fs::read(seed.join("one/a.txt")).unwrap(),
        b"first moved work"
    );
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
}

#[test]
fn retried_sibling_directories_keep_independent_undo_membership_when_one_return_is_skipped() {
    let source = tempfile::tempdir().unwrap();
    let destination = other_filesystem(source.path());
    let original = source.path().join("project");
    for name in ["one", "two"] {
        fs::create_dir_all(original.join(name)).unwrap();
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            original.join(name).join("blocked"),
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .unwrap();
    }
    fs::write(original.join("z.txt"), b"earlier moved work").unwrap();
    let history = source.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![original.clone()],
                destination: destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(moved.failures.len(), 2);
    for (name, contents) in [
        ("one", b"first retried work".as_slice()),
        ("two", b"second retried work".as_slice()),
    ] {
        let file = original.join(name).join("blocked");
        fs::remove_file(&file).unwrap();
        fs::write(file, contents).unwrap();
    }
    let retried = engine
        .resume_transfer(&moved.retry_continuation.unwrap(), &cancel, |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(retried.failures.is_empty());
    assert!(!original.exists());
    fs::create_dir_all(original.join("one")).unwrap();
    fs::write(original.join("one/newer.txt"), b"newer first-folder work").unwrap();
    let partial = engine
        .undo_interactive(None, &cancel, |question| {
            if question.destination == original {
                UndoChoice::Merge
            } else {
                assert_eq!(question.destination, original.join("one"));
                UndoChoice::Skip
            }
        })
        .unwrap();
    assert!(partial.remaining > 0);
    assert_eq!(
        fs::read(original.join("two/blocked")).unwrap(),
        b"second retried work"
    );
    let current = destination.path().join("project");
    assert_eq!(
        fs::read(current.join("one/blocked")).unwrap(),
        b"first retried work"
    );
    let held = destination.path().join("completed-second-container");
    fs::rename(current.join("two"), &held).unwrap();
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(
                question.destination,
                original.join("one"),
                "a completed sibling must not remain a parent of the skipped return"
            );
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert_eq!(
        fs::read(original.join("one (2)/blocked")).unwrap(),
        b"first retried work"
    );
    let finished = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.incoming.path(), current.join("two"));
            UndoChoice::FinishStep
        })
        .unwrap();
    assert_eq!(finished.remaining, 1);
    assert_eq!(finished.unremoved, vec![current.join("two")]);
    engine.undo(&cancel).unwrap();
    assert!(!current.exists());
    assert!(held.is_dir());
    assert_eq!(
        fs::read(original.join("z.txt")).unwrap(),
        b"earlier moved work"
    );
    assert_eq!(
        fs::read(original.join("one/newer.txt")).unwrap(),
        b"newer first-folder work"
    );
}

#[test]
fn a_directory_retry_preserves_a_newer_source_container_until_the_recorded_container_returns() {
    let source = tempfile::tempdir().unwrap();
    let destination = other_filesystem(source.path());
    let original = source.path().join("project");
    fs::create_dir_all(original.join("nested")).unwrap();
    let blocked = original.join("nested/blocked");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &blocked,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    fs::write(original.join("z.txt"), b"already moved work").unwrap();
    let current = destination.path().join("project");
    let engine = OperationEngine::open(source.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![original.clone()],
                destination: destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(moved.failures.len(), 1);
    let held = source.path().join("original-container-held");
    fs::rename(&original, &held).unwrap();
    fs::create_dir_all(original.join("nested")).unwrap();
    fs::write(&blocked, b"newer source work").unwrap();
    let newer_identity = fs::metadata(&original).unwrap().ino();
    let refused = engine
        .resume_transfer(&moved.retry_continuation.unwrap(), &cancel, |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(!refused.failures.is_empty());
    assert_eq!(
        fs::read(&blocked).unwrap(),
        b"newer source work",
        "retry must not move files from a replacement source container"
    );
    assert!(!current.join("nested/blocked").exists());
    let newer_held = source.path().join("newer-container-held");
    fs::rename(&original, &newer_held).unwrap();
    fs::rename(&held, &original).unwrap();
    fs::remove_file(&blocked).unwrap();
    fs::write(&blocked, b"repaired original work").unwrap();
    let completed = engine
        .resume_transfer(&refused.retry_continuation.unwrap(), &cancel, |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(completed.failures.is_empty());
    assert!(!original.exists());
    assert_eq!(
        fs::read(current.join("nested/blocked")).unwrap(),
        b"repaired original work"
    );
    assert_eq!(fs::metadata(&newer_held).unwrap().ino(), newer_identity);
    assert_eq!(
        fs::read(newer_held.join("nested/blocked")).unwrap(),
        b"newer source work"
    );
}

#[test]
fn a_directory_retry_waits_for_its_original_destination_container_before_copying_or_removing_source_data()
 {
    let source = tempfile::tempdir().unwrap();
    let destination = other_filesystem(source.path());
    let original = source.path().join("project");
    fs::create_dir_all(original.join("nested")).unwrap();
    let blocked = original.join("nested/blocked");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &blocked,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    fs::write(original.join("z.txt"), b"already moved work").unwrap();
    let current = destination.path().join("project");
    let engine = OperationEngine::open(source.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![original.clone()],
                destination: destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(moved.failures.len(), 1);
    fs::remove_file(&blocked).unwrap();
    fs::write(&blocked, b"repaired source work").unwrap();
    let held = destination.path().join("original-destination-held");
    fs::rename(&current, &held).unwrap();
    fs::create_dir_all(current.join("nested")).unwrap();
    fs::write(current.join("newer.txt"), b"newer destination work").unwrap();
    let newer_identity = fs::metadata(&current).unwrap().ino();
    let refused = engine
        .resume_transfer(&moved.retry_continuation.unwrap(), &cancel, |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(!refused.failures.is_empty());
    assert_eq!(
        fs::read(&blocked).unwrap(),
        b"repaired source work",
        "source data must stay until the original destination container returns"
    );
    assert!(!current.join("nested/blocked").exists());
    assert_eq!(fs::read(held.join("z.txt")).unwrap(), b"already moved work");
    let newer_held = destination.path().join("newer-destination-held");
    fs::rename(&current, &newer_held).unwrap();
    fs::rename(&held, &current).unwrap();
    let completed = engine
        .resume_transfer(&refused.retry_continuation.unwrap(), &cancel, |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(completed.failures.is_empty());
    assert!(!original.exists());
    assert_eq!(
        fs::read(current.join("nested/blocked")).unwrap(),
        b"repaired source work"
    );
    assert_eq!(fs::metadata(&newer_held).unwrap().ino(), newer_identity);
    assert_eq!(
        fs::read(newer_held.join("newer.txt")).unwrap(),
        b"newer destination work"
    );
}

#[test]
fn retry_failed_and_continue_remaining_share_directory_completion_in_either_order_after_cancellation()
 {
    use canopa_files::transfers::TransferEvent;
    for retry_first in [false, true] {
        for after_file in [false, true] {
            let source = tempfile::tempdir().unwrap();
            let destination = other_filesystem(source.path());
            let original = source.path().join("project");
            fs::create_dir(&original).unwrap();
            let failed = original.join("a-blocked");
            rustix::fs::mkfifoat(
                rustix::fs::CWD,
                &failed,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            )
            .unwrap();
            let later = original.join("b.txt");
            fs::write(&later, b"later work").unwrap();
            let current = destination.path().join("project");
            let engine = OperationEngine::open(source.path().join("state/history")).unwrap();
            let cancel = Cancellation::default();
            let paused = engine.transfer(&TransferRequest { kind: TransferKind::Move, sources: vec![original.clone()], destination: destination.path().to_owned() }, &Cancellation::default(), |event| {
                if matches!(event, TransferEvent::Progress { ref path, .. } if path == &later && later.exists() != after_file) {
                    TransferReply::Cancel
                } else { TransferReply::Continue }
            }).unwrap();
            assert!(paused.cancelled);
            assert_eq!(paused.failures.len(), 1);
            fs::remove_file(&failed).unwrap();
            fs::write(&failed, b"repaired work").unwrap();
            let retry = paused.retry_continuation.unwrap();
            let remaining = paused.continuation.unwrap();
            let plans = if retry_first {
                [&retry, &remaining]
            } else {
                [&remaining, &retry]
            };
            for plan in plans {
                let result = engine
                    .resume_transfer(plan, &cancel, |_| TransferReply::Continue)
                    .unwrap();
                assert!(
                    result.failures.is_empty(),
                    "retry_first={retry_first}, after_file={after_file}: {:?}",
                    result.failures
                );
                assert!(!result.cancelled && !result.needs_attention);
            }
            assert!(
                !original.exists(),
                "retry_first={retry_first}, after_file={after_file}"
            );
            assert_eq!(
                fs::read(current.join("a-blocked")).unwrap(),
                b"repaired work"
            );
            assert_eq!(fs::read(current.join("b.txt")).unwrap(), b"later work");
            for _ in 0..4 {
                if engine.latest_undo_id().is_none() {
                    break;
                }
                engine.undo_interactive(None, &cancel, |_| panic!("captured directory completion and data must undo without asking about unrelated versions")).unwrap();
            }
            assert!(engine.latest_undo_id().is_none());
            assert!(!current.exists());
            assert_eq!(fs::read(&failed).unwrap(), b"repaired work");
            assert_eq!(fs::read(&later).unwrap(), b"later work");
        }
    }
}

#[test]
fn cancellation_after_a_destination_edit_keeps_failed_retry_separate_from_remaining_completion() {
    use canopa_files::transfers::{ConflictChoice, TransferEvent};
    for retry_first in [false, true] {
        let source = tempfile::tempdir().unwrap();
        let destination = other_filesystem(source.path());
        let original = source.path().join("project");
        fs::create_dir(&original).unwrap();
        let file = original.join("b.txt");
        fs::write(&file, b"original source work").unwrap();
        let current = destination.path().join("project");
        let engine = OperationEngine::open(source.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        let paused = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![original.clone()],
                    destination: destination.path().to_owned(),
                },
                &Cancellation::default(),
                |event| {
                    if let TransferEvent::Verifying {
                        source,
                        destination,
                    } = event
                    {
                        assert_eq!(source, file);
                        fs::write(destination, b"newer destination work").unwrap();
                        TransferReply::Cancel
                    } else {
                        TransferReply::Continue
                    }
                },
            )
            .unwrap();
        assert!(paused.cancelled);
        assert_eq!(paused.failures.len(), 1);
        assert_eq!(
            paused.remaining,
            vec![original.clone()],
            "a failed leaf must belong to Retry rather than also appearing in Continue"
        );
        let retry = paused
            .retry_continuation
            .expect("the preserved changed destination must retain an exact failed-item plan");
        let remaining = paused.continuation.unwrap();
        let plans = if retry_first {
            [&retry, &remaining]
        } else {
            [&remaining, &retry]
        };
        for plan in plans {
            let result = engine
                .resume_transfer(plan, &cancel, |_| TransferReply::Resolve {
                    choice: ConflictChoice::KeepBoth,
                    apply_to_remaining: false,
                })
                .unwrap();
            assert!(result.failures.is_empty(), "{:?}", result.failures);
        }
        assert!(!original.exists());
        assert_eq!(
            fs::read(current.join("b.txt")).unwrap(),
            b"newer destination work"
        );
        assert_eq!(
            fs::read(current.join("b (2).txt")).unwrap(),
            b"original source work"
        );
        engine
            .undo_interactive(None, &cancel, |_| {
                panic!("Undo of the retry must return only its original source version")
            })
            .unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"original source work");
        assert_eq!(
            fs::read(current.join("b.txt")).unwrap(),
            b"newer destination work"
        );
        assert!(!current.join("b (2).txt").exists());
    }
}

#[test]
fn retrying_a_selected_file_never_claims_its_unselected_parent_directory() {
    let source = tempfile::tempdir().unwrap();
    let destination = other_filesystem(source.path());
    let parent = source.path().join("user-folder");
    fs::create_dir(&parent).unwrap();
    let file = parent.join("blocked");
    fs::write(&file, b"repaired selected file").unwrap();
    let identity = fs::metadata(&parent).unwrap().ino();
    let engine = OperationEngine::open(source.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let observed = TransferRequest {
        kind: TransferKind::Move,
        sources: vec![file.clone()],
        destination: destination.path().to_owned(),
    }
    .observe_sources()
    .unwrap();
    let unavailable = source.path().join("unavailable parent");
    fs::rename(&parent, &unavailable).unwrap();
    let failed = engine
        .transfer_observed_with_check(
            &observed,
            &cancel,
            |_, _| Ok(()),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(failed.failures.len(), 1);
    fs::rename(&unavailable, &parent).unwrap();
    let result = engine
        .resume_transfer(&failed.retry_continuation.unwrap(), &cancel, |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert!(result.failures.is_empty());
    assert_eq!(fs::metadata(&parent).unwrap().ino(), identity);
    assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
    assert_eq!(
        fs::read(destination.path().join("blocked")).unwrap(),
        b"repaired selected file"
    );
    engine.undo(&cancel).unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"repaired selected file");
    assert_eq!(fs::metadata(&parent).unwrap().ino(), identity);
}

#[test]
fn a_directory_retry_rechecks_container_identity_after_a_verified_copy_even_if_the_file_node_is_unchanged()
 {
    use canopa_files::transfers::TransferEvent;
    for source_side in [false, true] {
        let source = tempfile::tempdir().unwrap();
        let destination = other_filesystem(source.path());
        let original = source.path().join("project");
        fs::create_dir_all(original.join("nested")).unwrap();
        let blocked = original.join("nested/blocked");
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            &blocked,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .unwrap();
        fs::write(original.join("z.txt"), b"already moved work").unwrap();
        let current = destination.path().join("project");
        let engine = OperationEngine::open(source.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        let moved = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![original.clone()],
                    destination: destination.path().to_owned(),
                },
                &cancel,
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert_eq!(moved.failures.len(), 1);
        fs::remove_file(&blocked).unwrap();
        fs::write(&blocked, b"repaired work").unwrap();
        let mut changed = false;
        let refused = engine
            .resume_transfer(&moved.retry_continuation.unwrap(), &cancel, |event| {
                if let TransferEvent::Verifying {
                    source,
                    destination,
                } = event
                {
                    assert_eq!(source, blocked);
                    let container = if source_side { &original } else { &current };
                    let file = if source_side { &source } else { &destination };
                    let identity = fs::metadata(file).unwrap().ino();
                    let held = container.with_file_name("container-held-after-copy");
                    fs::rename(container, &held).unwrap();
                    fs::create_dir_all(container.join("nested")).unwrap();
                    fs::rename(held.join("nested/blocked"), file).unwrap();
                    assert_eq!(fs::metadata(file).unwrap().ino(), identity);
                    fs::write(container.join("newer.txt"), b"newer container work").unwrap();
                    changed = true;
                }
                TransferReply::Continue
            })
            .unwrap();
        assert!(changed);
        assert!(!refused.failures.is_empty());
        assert_eq!(
            fs::read(&blocked).unwrap(),
            b"repaired work",
            "the unchanged file identity must not bypass its captured container identity"
        );
        let container = if source_side { &original } else { &current };
        assert_eq!(
            fs::read(container.join("newer.txt")).unwrap(),
            b"newer container work"
        );
    }
}

#[test]
fn a_paused_folder_copy_waits_for_its_original_containers_and_keeps_its_chosen_destination() {
    use canopa_files::transfers::{ConflictChoice, TransferEvent};
    for source_side in [false, true] {
        let source = tempfile::tempdir().unwrap();
        let destination = other_filesystem(source.path());
        let original = source.path().join("project");
        fs::create_dir(&original).unwrap();
        fs::write(original.join("a.txt"), b"first copied work").unwrap();
        fs::write(original.join("b.txt"), b"remaining copied work").unwrap();
        let existing = destination.path().join("project");
        fs::create_dir(&existing).unwrap();
        fs::write(existing.join("existing.txt"), b"existing destination work").unwrap();
        let kept = destination.path().join("project (2)");
        let engine = OperationEngine::open(source.path().join("state/history")).unwrap();
        let cancel = Cancellation::default();
        let paused = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![original.clone()],
                    destination: destination.path().to_owned(),
                },
                &Cancellation::default(),
                |event| match event {
                    TransferEvent::Conflict { folders: true, .. } => TransferReply::Resolve {
                        choice: ConflictChoice::KeepBoth,
                        apply_to_remaining: false,
                    },
                    TransferEvent::Progress { path, .. }
                        if path == original.join("a.txt") && kept.join("a.txt").exists() =>
                    {
                        TransferReply::Cancel
                    }
                    _ => TransferReply::Continue,
                },
            )
            .unwrap();
        assert!(paused.cancelled && paused.failures.is_empty());
        let container = if source_side { &original } else { &kept };
        let held = container.with_file_name("original-container-held");
        fs::rename(container, &held).unwrap();
        fs::create_dir(container).unwrap();
        fs::write(container.join("newer.txt"), b"newer container work").unwrap();
        if source_side {
            fs::write(container.join("b.txt"), b"newer source work").unwrap();
        }
        let refused = engine
            .resume_transfer(&paused.continuation.unwrap(), &cancel, |_| {
                TransferReply::Continue
            })
            .unwrap();
        assert!(
            !refused.failures.is_empty(),
            "paused copies must retain directory identity as well as destination names"
        );
        assert!(!kept.join("b.txt").exists());
        let newer_held = container.with_file_name("newer-container-held");
        fs::rename(container, &newer_held).unwrap();
        fs::rename(&held, container).unwrap();
        let completed = engine
            .resume_transfer(&refused.retry_continuation.unwrap(), &cancel, |_| {
                TransferReply::Continue
            })
            .unwrap();
        assert!(completed.failures.is_empty());
        assert_eq!(fs::read(kept.join("a.txt")).unwrap(), b"first copied work");
        assert_eq!(
            fs::read(kept.join("b.txt")).unwrap(),
            b"remaining copied work"
        );
        assert_eq!(
            fs::read(original.join("a.txt")).unwrap(),
            b"first copied work"
        );
        assert_eq!(
            fs::read(original.join("b.txt")).unwrap(),
            b"remaining copied work"
        );
        engine.undo(&cancel).unwrap();
        assert!(!kept.join("b.txt").exists());
        engine.undo(&cancel).unwrap();
        assert!(!kept.exists());
        assert_eq!(
            fs::read(existing.join("existing.txt")).unwrap(),
            b"existing destination work"
        );
        assert_eq!(
            fs::read(newer_held.join("newer.txt")).unwrap(),
            b"newer container work"
        );
    }
}

#[test]
fn retrying_a_failed_folder_copy_never_removes_an_empty_source_folder() {
    let source = tempfile::tempdir().unwrap();
    let destination = other_filesystem(source.path());
    let original = source.path().join("project");
    fs::create_dir(&original).unwrap();
    let identity = fs::metadata(&original).unwrap().ino();
    let blocked = original.join("blocked");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&blocked)
            .status()
            .unwrap()
            .success()
    );
    let engine = OperationEngine::open(source.path().join("state/history")).unwrap();
    let cancel = Cancellation::default();
    let failed = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![original.clone()],
                destination: destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(failed.failures.len(), 1);
    fs::remove_file(&blocked).unwrap();
    let retry = engine
        .resume_transfer(&failed.retry_continuation.unwrap(), &cancel, |_| {
            TransferReply::Continue
        })
        .unwrap();
    assert_eq!(retry.failures.len(), 1);
    assert_eq!(fs::metadata(&original).unwrap().ino(), identity);
    assert!(fs::read_dir(&original).unwrap().next().is_none());
    engine.undo(&cancel).unwrap();
    assert!(!destination.path().join("project").exists());
    assert_eq!(fs::metadata(&original).unwrap().ino(), identity);
}

#[test]
fn skip_and_cancel_preserve_a_whole_moved_directory_for_a_later_attempt() {
    let fixture = Fixture::copied_tree();
    fs::create_dir(&fixture.original).unwrap();
    fs::write(fixture.original.join("newer.txt"), b"newer work").unwrap();
    for choice in [UndoChoice::Skip, UndoChoice::Cancel] {
        let mut questions = 0;
        let result = fixture
            .engine
            .undo_interactive(None, &Cancellation::default(), |question| {
                questions += 1;
                assert_eq!(question.destination, fixture.original);
                choice
            })
            .unwrap();
        assert_eq!(questions, 1);
        assert_eq!(result.cancelled, choice == UndoChoice::Cancel);
        assert!(result.remaining > 0);
        assert_eq!(
            fs::read(fixture.current.join("a.txt")).unwrap(),
            b"older copied a"
        );
        assert!(!fixture.original.join("a.txt").exists());
    }
    let cancel = Cancellation::default();
    let result = fixture
        .engine
        .undo_interactive(None, &cancel, |_| {
            cancel.cancel();
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert!(result.cancelled);
    assert!(!fixture.original.with_file_name("project (2)").exists());
    drop(fixture.engine);
    let engine = OperationEngine::open(&fixture.history).unwrap();
    let result = engine
        .undo_interactive(None, &Cancellation::default(), |question| {
            assert_eq!(question.destination, fixture.original);
            UndoChoice::Merge
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert!(!fixture.current.exists());
    assert_eq!(
        fs::read(fixture.original.join("a.txt")).unwrap(),
        b"older copied a"
    );
    assert_eq!(
        fs::read(fixture.original.join("newer.txt")).unwrap(),
        b"newer work"
    );
}

#[test]
fn a_replaced_original_folder_invalidates_the_pending_merge_or_keep_both_choice() {
    for choice in [UndoChoice::Merge, UndoChoice::KeepBoth] {
        let fixture = Fixture::copied_tree();
        fs::create_dir(&fixture.original).unwrap();
        fs::write(fixture.original.join("newer.txt"), b"reviewed newer work").unwrap();
        let elsewhere = fixture.original.with_file_name("newer moved elsewhere");
        let error = fixture
            .engine
            .undo_interactive(None, &Cancellation::default(), |question| {
                assert_eq!(question.destination, fixture.original);
                fs::rename(&fixture.original, &elsewhere).unwrap();
                fs::create_dir(&fixture.original).unwrap();
                fs::write(
                    fixture.original.join("replacement.txt"),
                    b"replacement work",
                )
                .unwrap();
                choice
            })
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock);
        assert_eq!(
            fs::read(fixture.current.join("a.txt")).unwrap(),
            b"older copied a"
        );
        assert!(!fixture.original.join("a.txt").exists());
        assert!(!fixture.original.with_file_name("project (2)").exists());
        drop(fixture.engine);
        let engine = OperationEngine::open(&fixture.history).unwrap();
        let result = engine
            .undo_interactive(None, &Cancellation::default(), |_| UndoChoice::Merge)
            .unwrap();
        assert_eq!(result.remaining, 0);
        assert_eq!(
            fs::read(fixture.original.join("a.txt")).unwrap(),
            b"older copied a"
        );
        assert_eq!(
            fs::read(fixture.original.join("replacement.txt")).unwrap(),
            b"replacement work"
        );
        assert_eq!(
            fs::read(elsewhere.join("newer.txt")).unwrap(),
            b"reviewed newer work"
        );
        assert!(
            engine
                .interrupted_operations(&Cancellation::default())
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn skipping_one_directory_in_a_shared_destination_batch_does_not_reask_or_remove_its_source() {
    use std::{sync::mpsc, time::Duration};
    let source = tempfile::tempdir().unwrap();
    let device = fs::metadata(source.path()).unwrap().dev();
    let destination = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|path| {
            (fs::metadata(&path).ok()?.dev() != device)
                .then(|| {
                    tempfile::Builder::new()
                        .prefix(".canopa-directory-batch-")
                        .tempdir_in(path)
                        .ok()
                })
                .flatten()
        })
        .unwrap();
    let first = source.path().join("one/project");
    let second = source.path().join("two/project");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    fs::write(first.join("a.txt"), b"first moved work").unwrap();
    fs::write(second.join("b.txt"), b"second moved work").unwrap();
    let history = source.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![first.clone(), second.clone()],
                destination: destination.path().to_owned(),
            },
            &Cancellation::default(),
            |_| TransferReply::Resolve {
                choice: canopa_files::transfers::ConflictChoice::Merge,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    fs::create_dir(&first).unwrap();
    fs::create_dir(&second).unwrap();
    fs::write(first.join("newer.txt"), b"first newer work").unwrap();
    fs::write(second.join("newer.txt"), b"second newer work").unwrap();
    let cancel = Cancellation::default();
    let deadline = cancel.clone();
    let (done, received) = mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if received.recv_timeout(Duration::from_secs(10)).is_err() {
            deadline.cancel()
        }
    });
    let mut second_questions = 0;
    let partial = engine
        .undo_interactive(None, &cancel, |question| {
            if question.destination == second {
                second_questions += 1;
                assert_eq!(
                    second_questions, 1,
                    "Skip must apply to the entire folder for this attempt"
                );
                UndoChoice::Skip
            } else {
                assert_eq!(question.destination, first);
                UndoChoice::Merge
            }
        })
        .unwrap();
    let _ = done.send(());
    watchdog.join().unwrap();
    assert!(!partial.cancelled);
    assert!(partial.remaining > 0);
    assert_eq!(fs::read(first.join("a.txt")).unwrap(), b"first moved work");
    let current = destination.path().join("project");
    assert_eq!(
        fs::read(current.join("b.txt")).unwrap(),
        b"second moved work"
    );
    drop(engine);
    let engine = OperationEngine::open(history).unwrap();
    let result = engine
        .undo_interactive(None, &Cancellation::default(), |question| {
            assert_eq!(question.destination, second);
            UndoChoice::Merge
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert!(!current.exists());
    assert_eq!(
        fs::read(second.join("b.txt")).unwrap(),
        b"second moved work"
    );
    assert_eq!(
        fs::read(first.join("newer.txt")).unwrap(),
        b"first newer work"
    );
    assert_eq!(
        fs::read(second.join("newer.txt")).unwrap(),
        b"second newer work"
    );
}

#[test]
fn keeping_both_returns_a_cross_filesystem_directory_and_preserves_its_older_owned_ancestry() {
    let mut fixture = Fixture::copied_tree();
    let cancel = Cancellation::default();
    fs::create_dir(&fixture.original).unwrap();
    fs::write(fixture.original.join("newer.txt"), b"newer folder work").unwrap();
    let newer_identity = fs::metadata(&fixture.original).unwrap().ino();
    let returned = fixture.original.with_file_name("project (2)");
    let mut questions = 0;
    let result = fixture
        .engine
        .undo_interactive(None, &cancel, |question| {
            questions += 1;
            assert_eq!(question.destination, fixture.original);
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(questions, 1);
    assert_eq!(result.remaining, 0);
    assert_eq!(
        result.completed, 6,
        "both file returns and their cleanup steps, plus both directory steps, were undone"
    );
    assert!(!fixture.current.exists());
    assert_eq!(fs::read(returned.join("a.txt")).unwrap(), b"older copied a");
    assert_eq!(fs::read(returned.join("z.txt")).unwrap(), b"older copied z");
    drop(fixture.engine);
    fixture.engine = OperationEngine::open(&fixture.history).unwrap();
    fixture
        .engine
        .undo_interactive(None, &cancel, |_| {
            panic!("owned returned copy should undo without adopting the newer folder")
        })
        .unwrap();
    assert!(!returned.exists());
    assert_eq!(
        fs::metadata(&fixture.original).unwrap().ino(),
        newer_identity
    );
    assert_eq!(
        fs::read(fixture.original.join("newer.txt")).unwrap(),
        b"newer folder work"
    );
}

#[test]
fn undo_of_a_cross_filesystem_directory_move_can_merge_into_a_newer_original_folder() {
    let source = tempfile::tempdir().unwrap();
    let device = fs::metadata(source.path()).unwrap().dev();
    let destination = [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|path| {
            (fs::metadata(&path).ok()?.dev() != device)
                .then(|| {
                    tempfile::Builder::new()
                        .prefix(".canopa-directory-move-")
                        .tempdir_in(path)
                        .ok()
                })
                .flatten()
        })
        .expect("two writable filesystems");
    let original = source.path().join("project");
    fs::create_dir(&original).unwrap();
    let seed = source.path().join("a.txt");
    fs::write(&seed, b"older copied work").unwrap();
    let history = source.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let copied = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![seed.clone()],
                destination: original.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(copied.failures.is_empty());
    fs::write(original.join("z.txt"), b"incoming final work").unwrap();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![original.clone()],
                destination: destination.path().to_owned(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(moved.failures.is_empty());
    assert!(!original.exists());
    let current = destination.path().join("project");
    fs::create_dir(&original).unwrap();
    fs::write(original.join("z.txt"), b"newer original work").unwrap();
    let identity = fs::metadata(&original).unwrap().ino();
    let mut root_questions = 0;
    let partial = engine
        .undo_interactive(None, &cancel, |question| {
            if question.destination == original {
                root_questions += 1;
                assert!(question.can_merge());
                assert_eq!(question.incoming.path(), current);
                UndoChoice::Merge
            } else {
                assert_eq!(question.destination, original.join("z.txt"));
                UndoChoice::Skip
            }
        })
        .unwrap();
    assert_eq!(root_questions, 1);
    assert!(partial.remaining > 0);
    assert_eq!(
        fs::read(original.join("a.txt")).unwrap(),
        b"older copied work"
    );
    assert_eq!(
        fs::read(current.join("z.txt")).unwrap(),
        b"incoming final work"
    );
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let result = engine
        .undo_interactive(None, &cancel, |question| {
            assert_eq!(question.destination, original.join("z.txt"));
            UndoChoice::KeepBoth
        })
        .unwrap();
    assert_eq!(result.remaining, 0);
    assert!(!current.exists());
    assert_eq!(fs::metadata(&original).unwrap().ino(), identity);
    assert_eq!(
        fs::read(original.join("z.txt")).unwrap(),
        b"newer original work"
    );
    assert_eq!(
        fs::read(original.join("z (2).txt")).unwrap(),
        b"incoming final work"
    );
    engine.undo(&cancel).unwrap();
    assert!(!original.join("a.txt").exists());
    assert_eq!(fs::read(seed).unwrap(), b"older copied work");
    assert_eq!(
        fs::read(original.join("z.txt")).unwrap(),
        b"newer original work"
    );
    assert!(engine.interrupted_operations(&cancel).unwrap().is_empty());
}
