//! Deterministic filesystem races at execution capture and the real rename
//! syscall. No environment switch or hook is built into the app.
use super::*;
use std::cell::RefCell;
mod claim_review;

type Hook = Box<dyn FnOnce(&Path, &Path)>;
type CaptureHook = Box<dyn FnOnce(&Path, &Path) + Send>;
static CAPTURE_HOOKS: Mutex<std::collections::BTreeMap<PathBuf, CaptureHook>> =
    Mutex::new(std::collections::BTreeMap::new());

pub(super) fn before_capture(source: &Path, destination: &Path) {
    let hook = CAPTURE_HOOKS.lock().unwrap().remove(source);
    if let Some(hook) = hook {
        hook(source, destination);
    }
}

#[cfg(feature = "local-runtime")]
struct CaptureReset(PathBuf);
#[cfg(feature = "local-runtime")]
impl Drop for CaptureReset {
    fn drop(&mut self) {
        CAPTURE_HOOKS.lock().unwrap().remove(&self.0);
    }
}

#[cfg(feature = "local-runtime")]
#[test]
fn prepared_leaf_replacement_before_engine_capture_does_not_gain_new_authority() {
    prepared_capture_boundary(false);
}

#[cfg(feature = "local-runtime")]
#[test]
fn prepared_parent_replacement_before_engine_capture_does_not_gain_new_authority() {
    prepared_capture_boundary(true);
}

#[cfg(feature = "local-runtime")]
fn prepared_capture_boundary(replace_parent: bool) {
    use crate::operation_jobs::{OperationJobs, Phase};
    use std::{
        ffi::OsString,
        time::{Duration, Instant},
    };
    fn wait<T>(mut probe: impl FnMut() -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(value) = probe() {
                return value;
            }
            assert!(Instant::now() < deadline, "private queue did not settle");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let root = tempfile::Builder::new()
        .prefix("canopa-rename-capture-")
        .tempdir()
        .unwrap();
    let parent = root.path().join("work");
    fs::create_dir(&parent).unwrap();
    let source = parent.join("source");
    let target = parent.join("renamed");
    let parked = root.path().join("parked");
    fs::write(&source, b"Original prepared selection").unwrap();
    let identity = Identity::read(&source).unwrap();
    let jobs = OperationJobs::start(
        root.path().join("history"),
        root.path().join("volumes.json"),
    )
    .unwrap();
    wait(|| {
        let state = jobs.snapshot();
        assert!(state.error.is_none(), "{:?}", state.error);
        (!state.initializing).then_some(())
    });
    let mut preparation = jobs.prepare_rename(source.clone(), None).unwrap();
    let prepared = wait(|| preparation.try_recv().unwrap()).unwrap();
    let parked_at_gate = parked.clone();
    let parent_at_gate = parent.clone();
    let _reset = CaptureReset(source.clone());
    assert!(
        CAPTURE_HOOKS
            .lock()
            .unwrap()
            .insert(
                source.clone(),
                Box::new(move |actual, _| {
                    assert!(identity.matches(actual));
                    if replace_parent {
                        fs::rename(&parent_at_gate, &parked_at_gate).unwrap();
                        fs::create_dir(&parent_at_gate).unwrap();
                    } else {
                        fs::rename(actual, &parked_at_gate).unwrap();
                    }
                    fs::write(actual, b"Replacement after queue validation").unwrap();
                })
            )
            .is_none()
    );
    let mut admission = jobs
        .submit_rename(prepared, OsString::from("renamed"))
        .unwrap();
    let id = wait(|| admission.try_recv().unwrap()).unwrap();
    let row = wait(|| {
        jobs.snapshot().jobs.into_iter().find(|row| {
            row.id == id
                && matches!(
                    row.phase,
                    Phase::Finished | Phase::Failed | Phase::SaveFailed | Phase::AwaitingReview
                )
        })
    });
    let mut shutdown = jobs.shutdown().unwrap();
    wait(|| shutdown.try_recv().unwrap()).unwrap();
    let parked_source = if replace_parent {
        parked.join("source")
    } else {
        parked
    };
    let observation = serde_json::json!({"parent_replacement":replace_parent,"row":format!("{row:?}"),"source_bytes":fs::read(&source).ok(),"target_bytes":fs::read(&target).ok(),"original_parked":identity.matches(&parked_source)});
    eprintln!("prepared-capture-boundary {observation}");
    fs::write(
        root.path().join("observation.json"),
        serde_json::to_vec_pretty(&observation).unwrap(),
    )
    .unwrap();
    if row.phase != Phase::Failed
        || fs::read(&source).ok().as_deref() != Some(b"Replacement after queue validation")
        || target.exists()
    {
        panic!(
            "prepared selection was replaced by engine capture; fixture preserved at {}",
            root.keep().display()
        );
    }
    assert!(identity.matches(&parked_source));
    assert_eq!(
        fs::read(&parked_source).unwrap(),
        b"Original prepared selection"
    );
}

thread_local! {
    static BEFORE_RENAME: RefCell<Option<Hook>> = const { RefCell::new(None) };
}

pub(super) fn before_rename(source: &Path, destination: &Path) {
    undo::abort_boundary::displaced_boundary::before_rename(source, destination);
    let hook = BEFORE_RENAME.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook(source, destination);
    }
}

pub(super) struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        BEFORE_RENAME.with(|slot| slot.borrow_mut().take());
    }
}

pub(super) fn install(hook: impl FnOnce(&Path, &Path) + 'static) -> Reset {
    BEFORE_RENAME.with(|slot| assert!(slot.borrow_mut().replace(Box::new(hook)).is_none()));
    Reset
}

fn after_claim(hook: impl FnOnce(&Path, &Path) + 'static) -> Reset {
    install(move |_, _| {
        BEFORE_RENAME.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
    })
}

#[test]
fn readonly_directory_rename_keeps_its_mode_without_requiring_a_different_parent() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::Builder::new()
        .prefix("canopa-rename-readonly-")
        .tempdir()
        .unwrap();
    let source = root.path().join("readonly");
    let target = root.path().join("renamed");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("child"), b"Original child").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o555)).unwrap();
    // The ordinary same-parent filesystem rename needs no permission change.
    fs::rename(&source, &target).unwrap();
    fs::rename(&target, &source).unwrap();
    let original = Identity::read(&source).unwrap();
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let result = engine.execute(
        OperationRequest::Rename {
            source: source.clone(),
            destination: target.clone(),
        },
        &Cancellation::default(),
    );
    if let Err(error) = result {
        panic!(
            "ordinary readonly rename failed after the plain syscall succeeded: {error}; preserved at {}",
            root.keep().display()
        );
    }
    assert!(original.matches(&target));
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o555
    );
    assert_eq!(fs::read(target.join("child")).unwrap(), b"Original child");
    engine.undo(&Cancellation::default()).unwrap();
    assert!(original.matches(&source));
    assert_eq!(
        fs::metadata(&source).unwrap().permissions().mode() & 0o777,
        0o555
    );
    fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn readonly_directory_claim_preserves_replacements_and_their_modes() {
    use std::os::unix::fs::PermissionsExt;
    for directory in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let target = root.path().join("renamed");
        let parked = root.path().join("parked");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("child"), b"Selected child").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o555)).unwrap();
        let original = Identity::read(&source).unwrap();
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        let parked_at_gate = parked.clone();
        let _reset = install(move |actual, _| {
            fs::rename(actual, parked_at_gate).unwrap();
            if directory {
                fs::create_dir(actual).unwrap();
                fs::write(actual.join("child"), b"Unselected child").unwrap();
                fs::set_permissions(actual, fs::Permissions::from_mode(0o555)).unwrap();
            } else {
                fs::write(actual, b"Unselected file").unwrap();
            }
        });
        assert!(
            engine
                .execute(
                    OperationRequest::Rename {
                        source: source.clone(),
                        destination: target.clone()
                    },
                    &Cancellation::default()
                )
                .is_err()
        );
        assert!(original.matches(&parked));
        assert!(!target.exists());
        if directory {
            assert_eq!(fs::read(source.join("child")).unwrap(), b"Unselected child");
            assert_eq!(
                fs::metadata(&source).unwrap().permissions().mode() & 0o777,
                0o555
            );
            fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).unwrap();
        } else {
            assert_eq!(fs::read(&source).unwrap(), b"Unselected file");
        }
        assert_eq!(
            fs::metadata(&parked).unwrap().permissions().mode() & 0o777,
            0o555
        );
        assert_eq!(fs::read(parked.join("child")).unwrap(), b"Selected child");
        fs::set_permissions(&parked, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn blocked_readonly_directory_claim_keeps_its_receipt_and_recovery_copy_after_reopening() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("renamed");
    let history = root.path().join("history");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("child"), b"Selected child").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o555)).unwrap();
    let original = Identity::read(&source).unwrap();
    let engine = OperationEngine::open(&history).unwrap();
    let source_at_gate = source.clone();
    let target_at_gate = target.clone();
    let _reset = after_claim(move |payload, _| {
        assert_eq!(fs::read(payload.join("child")).unwrap(), b"Selected child");
        fs::write(source_at_gate, b"Newer source occupant").unwrap();
        fs::write(target_at_gate, b"Newer destination occupant").unwrap();
    });
    assert!(
        engine
            .execute(
                OperationRequest::Rename {
                    source: source.clone(),
                    destination: target.clone()
                },
                &Cancellation::default()
            )
            .is_err()
    );
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let page = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap();
    assert_eq!(page.interrupted.len(), 1);
    assert_eq!(page.items.len(), 1);
    let item = &page.items[0];
    let staged = item.entry.as_ref().unwrap().path.clone();
    assert!(original.matches(&staged));
    assert_eq!(
        fs::metadata(&staged).unwrap().permissions().mode() & 0o777,
        0o555
    );
    assert!(page.interrupted[0].paths.contains(&staged));
    assert_eq!(
        engine
            .recover_copy(
                item,
                &staged.join("recursive-copy"),
                &Cancellation::default()
            )
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
    let recovered = root.path().join("recovered");
    engine
        .recover_copy(item, &recovered, &Cancellation::default())
        .unwrap();
    assert_eq!(
        fs::read(recovered.join("child")).unwrap(),
        b"Selected child"
    );
    assert_eq!(fs::read(staged.join("child")).unwrap(), b"Selected child");
    assert_eq!(fs::read(&source).unwrap(), b"Newer source occupant");
    assert_eq!(fs::read(&target).unwrap(), b"Newer destination occupant");
    assert!(original.matches(&staged));
    fs::set_permissions(&staged, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(&recovered, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn a_sibling_payload_in_a_preparation_journal_is_refused_without_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let history = root.path().join("history");
    let engine = OperationEngine::open(&history).unwrap();
    let (mut stage, temporary) = Stage::prepare(root.path()).unwrap();
    stage.sibling_payload = true;
    fs::write(stage.payload(), b"Must remain untouched").unwrap();
    {
        let mut state = engine.state.lock().unwrap();
        let id = state
            .begin_request("invalid preparation role", None)
            .unwrap();
        // Deliberately inject an unbound legacy/wrong-role journal. The live
        // creator API refuses to construct this preparation.
        let pending = Pending::Preparing {
            id,
            destination: root.path().join("destination"),
            stage: stage.clone(),
        };
        rusqlite::Connection::open(&history)
            .unwrap()
            .execute(
                "INSERT INTO pending VALUES(1,?1)",
                [serde_json::to_string(&pending).unwrap()],
            )
            .unwrap();
    }
    let _kept = temporary.keep();
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    assert!(engine.retry_recovery(&Cancellation::default()).is_err());
    assert_eq!(fs::read(stage.payload()).unwrap(), b"Must remain untouched");
    assert!(stage.identity.matches(&stage.directory));
    assert_eq!(
        engine
            .interrupted_operations(&Cancellation::default())
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn rename_claim_preserves_raw_names_directory_contents_and_final_symlinks_through_undo() {
    use std::ffi::OsString;
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    for kind in ["file", "directory", "symlink"] {
        let root = tempfile::tempdir().unwrap();
        let source = root
            .path()
            .join(OsString::from_vec(b"source-\xff".to_vec()));
        let target = root
            .path()
            .join(OsString::from_vec(b"target-\xfe".to_vec()));
        let outside = root.path().join("symlink-target");
        match kind {
            "directory" => {
                fs::create_dir(&source).unwrap();
                fs::write(source.join("child"), b"Nested original").unwrap();
            }
            "symlink" => {
                fs::write(&outside, b"Followed bytes stay untouched").unwrap();
                symlink("symlink-target", &source).unwrap();
            }
            _ => fs::write(&source, b"File original").unwrap(),
        }
        let identity = Identity::read(&source).unwrap();
        let history = root.path().join("history");
        let engine = OperationEngine::open(&history).unwrap();
        let summary = engine
            .execute(
                OperationRequest::Rename {
                    source: source.clone(),
                    destination: target.clone(),
                },
                &Cancellation::default(),
            )
            .unwrap();
        assert!(identity.matches(&target));
        assert!(fs::symlink_metadata(&source).is_err());
        drop(engine);
        let engine = OperationEngine::open(&history).unwrap();
        engine
            .undo_operation(summary.id, &Cancellation::default())
            .unwrap();
        assert!(identity.matches(&source));
        assert!(fs::symlink_metadata(&target).is_err());
        match kind {
            "directory" => assert_eq!(fs::read(source.join("child")).unwrap(), b"Nested original"),
            "symlink" => {
                assert_eq!(fs::read_link(&source).unwrap(), Path::new("symlink-target"));
                assert_eq!(fs::read(outside).unwrap(), b"Followed bytes stay untouched");
            }
            _ => assert_eq!(fs::read(&source).unwrap(), b"File original"),
        }
        assert!(!fs::read_dir(root.path()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .as_encoded_bytes()
                .starts_with(b".canopa-files-stage-")
        }));
    }
}

#[test]
fn schema23_completed_rename_and_queued_requests_upgrade_without_rewriting_their_bodies() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("renamed");
    fs::write(&source, b"Original version").unwrap();
    let history = root.path().join("history");
    let engine = OperationEngine::open(&history).unwrap();
    let queued = engine.queue_request(&"Original pending request").unwrap();
    engine
        .execute(
            OperationRequest::Rename {
                source: source.clone(),
                destination: target.clone(),
            },
            &Cancellation::default(),
        )
        .unwrap();
    drop(engine);
    let database = rusqlite::Connection::open(&history).unwrap();
    let before: String = database
        .query_row("SELECT body FROM changes", [], |row| row.get(0))
        .unwrap();
    database.pragma_update(None, "user_version", 23).unwrap();
    let engine = OperationEngine::open(&history).unwrap();
    let after: String = database
        .query_row("SELECT body FROM changes", [], |row| row.get(0))
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(engine.request_page(None).unwrap()[0].id(), queued.id());
    assert_eq!(
        engine.request_page(None).unwrap()[0]
            .payload::<String>()
            .unwrap(),
        "Original pending request"
    );
    assert_eq!(
        database
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        61
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(&source).unwrap(), b"Original version");
    assert!(!target.exists());
}

#[test]
fn aliased_paths_to_the_same_entry_do_not_create_a_rename_or_undo_record() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("work");
    let alias = root.path().join("alias");
    fs::create_dir(&parent).unwrap();
    symlink(&parent, &alias).unwrap();
    let target = parent.join("entry");
    let source = alias.join("entry");
    fs::write(&target, b"Same selected entry").unwrap();
    let identity = Identity::read(&target).unwrap();
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let result = engine.execute(
        OperationRequest::Rename {
            source,
            destination: target.clone(),
        },
        &Cancellation::default(),
    );
    if result.is_ok() || engine.latest_undo_id().is_some() {
        panic!(
            "same physical entry incorrectly accepted: {result:?}; preserved at {}",
            root.keep().display()
        );
    }
    assert!(identity.matches(&target));
    assert_eq!(fs::read(&target).unwrap(), b"Same selected entry");
    assert_eq!(fs::read_dir(&parent).unwrap().count(), 1);
}

#[test]
fn selected_entry_replaced_at_the_rename_boundary_is_returned_without_adoption() {
    use requests::RequestPhase;
    let root = tempfile::Builder::new()
        .prefix("canopa-rename-boundary-")
        .tempdir()
        .unwrap();
    let history = root.path().join("history");
    let source = root.path().join("source");
    let target = root.path().join("renamed");
    let parked = root.path().join("parked-original");
    fs::write(&source, b"Selected original").unwrap();
    let original_identity = Identity::read(&source).unwrap();
    let engine = OperationEngine::open(&history).unwrap();
    let queued = engine.queue_request(&"rename-boundary").unwrap();
    let running = engine
        .checkpoint_request(&queued, RequestPhase::Running, &"rename-boundary")
        .unwrap();
    let expected = source.clone();
    let parked_at_gate = parked.clone();
    let original = original_identity;
    let _reset = install(move |actual, _destination| {
        assert_eq!(actual, expected);
        assert!(original.matches(actual));
        let database = rusqlite::Connection::open_with_flags(
            &history,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let body: String = database
            .query_row("SELECT body FROM pending WHERE singleton=1", [], |row| {
                row.get(0)
            })
            .unwrap();
        let pending: Pending = serde_json::from_str(&body).unwrap();
        assert!(
            matches!(pending, Pending::Forward { change: Change::Rename { identity, .. }, .. } if identity == original)
        );
        let executions: u64 = database
            .query_row("SELECT COUNT(*) FROM request_executions", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(executions, 1);
        fs::rename(actual, parked_at_gate).unwrap();
        fs::write(actual, b"Unselected replacement").unwrap();
    });
    let result = engine.execution(&running).mutate(
        OperationRequest::Rename {
            source: source.clone(),
            destination: target.clone(),
        },
        &Cancellation::default(),
    );
    let source_bytes = fs::read(&source).ok();
    let target_bytes = fs::read(&target).ok();
    eprintln!(
        "rename-boundary result={result:?} source={source_bytes:?} target={target_bytes:?} original_parked={}",
        original_identity.matches(&parked)
    );
    if result.is_ok()
        || source_bytes.as_deref() != Some(b"Unselected replacement")
        || target.exists()
    {
        let preserved = root.keep();
        panic!(
            "replacement was adopted at the real Rename boundary; fixture preserved at {}",
            preserved.display()
        );
    }
    assert!(original_identity.matches(&parked));
    assert_eq!(fs::read(&parked).unwrap(), b"Selected original");
    assert!(engine.request_effects(&running).unwrap()[0].confirmed_steps == 0);
    assert_eq!(engine.latest_undo_id(), None);
}

#[test]
fn replaced_parent_at_the_rename_boundary_is_not_mutated() {
    let root = tempfile::Builder::new()
        .prefix("canopa-rename-parent-boundary-")
        .tempdir()
        .unwrap();
    let parent = root.path().join("work");
    let parked = root.path().join("parked-parent");
    fs::create_dir(&parent).unwrap();
    let source = parent.join("source");
    let target = parent.join("renamed");
    fs::write(&source, b"Selected original").unwrap();
    let identity = Identity::read(&source).unwrap();
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let original_parent = parent.clone();
    let parked_at_gate = parked.clone();
    let expected = source.clone();
    let _reset = install(move |actual, _destination| {
        assert_eq!(actual, expected);
        fs::rename(&original_parent, &parked_at_gate).unwrap();
        fs::create_dir(&original_parent).unwrap();
        fs::write(original_parent.join("source"), b"Replacement parent entry").unwrap();
    });
    let result = engine.execute(
        OperationRequest::Rename {
            source: source.clone(),
            destination: target.clone(),
        },
        &Cancellation::default(),
    );
    eprintln!(
        "rename-parent-boundary result={result:?} replacement_source={:?} target={:?}",
        fs::read(&source),
        fs::read(&target)
    );
    if result.is_ok()
        || fs::read(&source).ok().as_deref() != Some(b"Replacement parent entry")
        || target.exists()
    {
        let preserved = root.keep();
        panic!(
            "replacement parent was mutated; fixture preserved at {}",
            preserved.display()
        );
    }
    assert!(identity.matches(&parked.join("source")));
    assert!(!parked.join("renamed").exists());
    assert_eq!(engine.latest_undo_id(), None);
}

#[test]
fn destination_parent_replaced_during_publication_returns_through_the_opened_parents() {
    let root = tempfile::tempdir().unwrap();
    let destination = root.path().join("destination");
    let parked = root.path().join("parked-destination");
    fs::create_dir(&destination).unwrap();
    let source = root.path().join("source");
    let target = destination.join("renamed");
    fs::write(&source, b"Selected original").unwrap();
    let identity = Identity::read(&source).unwrap();
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let swapped = destination.clone();
    let parked_at_gate = parked.clone();
    let _reset = after_claim(move |payload, _| {
        assert_eq!(fs::read(payload).unwrap(), b"Selected original");
        fs::rename(&swapped, parked_at_gate).unwrap();
        fs::create_dir(&swapped).unwrap();
        fs::write(swapped.join("renamed"), b"Unselected destination").unwrap();
    });
    assert!(
        engine
            .execute(
                OperationRequest::Rename {
                    source: source.clone(),
                    destination: target.clone()
                },
                &Cancellation::default()
            )
            .is_err()
    );
    assert!(identity.matches(&source));
    assert_eq!(fs::read(&source).unwrap(), b"Selected original");
    assert_eq!(fs::read(&target).unwrap(), b"Unselected destination");
    assert!(!parked.join("renamed").exists());
    assert_eq!(fs::read_dir(&parked).unwrap().count(), 0);
    assert_eq!(engine.latest_undo_id(), None);
}

#[test]
fn unexpected_claim_with_a_blocked_return_survives_reopening_and_can_be_recovery_copied() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let parked = root.path().join("parked-original");
    let target = root.path().join("renamed");
    let history = root.path().join("history");
    fs::write(&source, b"Selected original").unwrap();
    let original = Identity::read(&source).unwrap();
    let engine = OperationEngine::open(&history).unwrap();
    let parked_at_gate = parked.clone();
    let _reset = install(move |actual, _| {
        fs::rename(actual, parked_at_gate).unwrap();
        fs::write(actual, b"Unexpected replacement").unwrap();
        BEFORE_RENAME.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(|_, original_path| {
                fs::write(original_path, b"Newer occupying entry").unwrap();
            }))
        });
    });
    assert!(
        engine
            .execute(
                OperationRequest::Rename {
                    source: source.clone(),
                    destination: target.clone()
                },
                &Cancellation::default()
            )
            .is_err()
    );
    assert_eq!(fs::read(&source).unwrap(), b"Newer occupying entry");
    assert!(original.matches(&parked));
    assert!(!target.exists());
    let page = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap();
    assert_eq!(page.interrupted.len(), 1);
    assert_eq!(page.items.len(), 1);
    let staged = page.items[0].entry.as_ref().unwrap().path.clone();
    let staged_identity = Identity::read(&staged).unwrap();
    assert_eq!(fs::read(&staged).unwrap(), b"Unexpected replacement");
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let page = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap();
    assert_eq!(page.interrupted.len(), 1);
    assert!(
        page.interrupted[0]
            .message
            .contains("preserved for Recovery review")
    );
    assert_eq!(page.items.len(), 1);
    assert!(staged_identity.matches(&staged));
    let recovered = root.path().join("recovered-copy");
    engine
        .recover_copy(&page.items[0], &recovered, &Cancellation::default())
        .unwrap();
    assert_eq!(fs::read(&recovered).unwrap(), b"Unexpected replacement");
    assert!(staged_identity.matches(&staged));
    assert!(original.matches(&parked));
    assert_eq!(fs::read(&source).unwrap(), b"Newer occupying entry");
    assert!(!target.exists());
}

#[test]
fn rename_claim_interrupted_in_a_real_child_is_preserved_without_replay() {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::ExitStatusExt;
    for directory in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::rename_boundary::rename_claim_crash_helper",
            ])
            .env("CANOPA_RENAME_CLAIM_ROOT", root.path())
            .env(
                "CANOPA_RENAME_CLAIM_DIRECTORY",
                if directory { "1" } else { "0" },
            )
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let staged: PathBuf =
            serde_json::from_slice(&fs::read(root.path().join("marker.json")).unwrap()).unwrap();
        let identity = Identity::read(&staged).unwrap();
        // A process ending cannot supply a user answer, move the claimed entry, or
        // let generic stage cleanup remove it, including after it has newer work.
        let edited = if directory {
            fs::set_permissions(&staged, fs::Permissions::from_mode(0o755)).unwrap();
            let child = staged.join("child");
            fs::write(&child, b"Edited after interruption").unwrap();
            fs::set_permissions(&staged, fs::Permissions::from_mode(0o555)).unwrap();
            child
        } else {
            fs::write(&staged, b"Edited after interruption").unwrap();
            staged.clone()
        };
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        let page = engine
            .recovery_items(None, &Cancellation::default())
            .unwrap();
        assert_eq!(page.interrupted.len(), 1);
        assert_eq!(page.items.len(), 1);
        assert!(identity.matches(&staged));
        assert_eq!(fs::read(&edited).unwrap(), b"Edited after interruption");
        assert!(!root.path().join("source").exists());
        assert!(!root.path().join("renamed").exists());
        assert_eq!(engine.latest_undo_id(), None);
        let unrelated = root.path().join("must-wait");
        assert!(
            engine
                .execute(
                    OperationRequest::CreateFolder {
                        path: unrelated.clone()
                    },
                    &Cancellation::default()
                )
                .is_err()
        );
        assert!(!unrelated.exists());
        assert!(identity.matches(&staged));
        assert!(page.items[0].can_keep_in_recovery());
        engine
            .keep_in_recovery(&page.items[0], &Cancellation::default())
            .unwrap();
        assert!(
            engine
                .interrupted_operations(&Cancellation::default())
                .unwrap()
                .is_empty()
        );
        assert_eq!(engine.latest_undo_id(), None);
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: unrelated.clone(),
                },
                &Cancellation::default(),
            )
            .unwrap();
        assert!(unrelated.is_dir());
        assert!(identity.matches(&staged));
        assert_eq!(fs::read(&edited).unwrap(), b"Edited after interruption");
        if directory {
            fs::set_permissions(&staged, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
}

#[test]
#[ignore = "parent starts this helper in a private directory and checks SIGKILL"]
fn rename_claim_crash_helper() {
    use std::os::unix::fs::PermissionsExt;
    let root = PathBuf::from(std::env::var_os("CANOPA_RENAME_CLAIM_ROOT").unwrap());
    let source = root.join("source");
    let directory = std::env::var("CANOPA_RENAME_CLAIM_DIRECTORY").unwrap() == "1";
    if directory {
        fs::create_dir(&source).unwrap();
        fs::write(source.join("child"), b"Selected original").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o555)).unwrap();
    } else {
        fs::write(&source, b"Selected original").unwrap();
    }
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let marker = root.join("marker.json");
    let _reset = after_claim(move |payload, _| {
        let data = if directory {
            payload.join("child")
        } else {
            payload.to_owned()
        };
        assert_eq!(fs::read(data).unwrap(), b"Selected original");
        fs::write(marker, serde_json::to_vec(payload).unwrap()).unwrap();
        std::process::Command::new("kill")
            .args(["-KILL", &std::process::id().to_string()])
            .status()
            .unwrap();
        panic!("the private helper must be killed");
    });
    engine
        .execute(
            OperationRequest::Rename {
                source,
                destination: root.join("renamed"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    panic!("the interruption hook did not run");
}
