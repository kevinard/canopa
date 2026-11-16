use super::*;
use std::cell::RefCell;

type Hook = Box<dyn FnOnce(&Path, &Path)>;
thread_local! {
    static BEFORE_REMOVE: RefCell<Option<Hook>> = const { RefCell::new(None) };
    static AT_PHASE: RefCell<Option<(&'static str, Hook)>> = const { RefCell::new(None) };
}
fn at_phase(point: &str, source: &Path, payload: &Path) {
    let hook = AT_PHASE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.as_ref().is_some_and(|(label, _)| *label == point) {
            slot.take().map(|(_, hook)| hook)
        } else {
            None
        }
    });
    if let Some(hook) = hook {
        hook(source, payload);
    }
}
pub(super) fn after_claim(source: &Path, payload: &Path) {
    at_phase("ready", source, payload);
}
pub(super) fn after_remove(source: &Path, payload: &Path) {
    at_phase("removed", source, payload);
}
pub(super) fn before_return(source: &Path, payload: &Path) {
    at_phase("returning", source, payload);
}
pub(super) fn after_return(source: &Path, payload: &Path) {
    at_phase("returned", source, payload);
}
pub(super) fn before_remove(source: &Path, payload: &Path) {
    if let Some(hook) = BEFORE_REMOVE.with(|slot| slot.borrow_mut().take()) {
        hook(source, payload);
    }
    at_phase("claimed", source, payload);
}
struct Reset;
impl Drop for Reset {
    fn drop(&mut self) {
        BEFORE_REMOVE.with(|slot| slot.borrow_mut().take());
        AT_PHASE.with(|slot| slot.borrow_mut().take());
    }
}
fn install(hook: impl FnOnce(&Path, &Path) + 'static) -> Reset {
    BEFORE_REMOVE.with(|slot| assert!(slot.borrow_mut().replace(Box::new(hook)).is_none()));
    Reset
}
fn output(source: &Path) -> tempfile::TempDir {
    let device = fs::metadata(source).unwrap().dev();
    [std::env::current_dir().unwrap(), PathBuf::from("/dev/shm")]
        .into_iter()
        .find_map(|parent| {
            (fs::metadata(&parent).ok()?.dev() != device)
                .then(|| {
                    tempfile::Builder::new()
                        .prefix(".canopa-source-claim-")
                        .tempdir_in(parent)
                        .ok()
                })
                .flatten()
        })
        .expect("two writable test filesystems")
}

#[test]
fn replaced_source_parent_is_never_used_for_the_source_claim() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("work");
    fs::create_dir(&parent).unwrap();
    let source = parent.join("item");
    fs::write(&source, b"Original copied source").unwrap();
    let target = output(root.path());
    let parked = root.path().join("original-parent");
    let moved_parent = parent.clone();
    let parked_at_gate = parked.clone();
    let _reset = rename_boundary::install(move |actual, claim| {
        assert_eq!(actual.file_name().unwrap(), "item");
        fs::rename(&moved_parent, &parked_at_gate).unwrap();
        fs::create_dir(&moved_parent).unwrap();
        fs::write(actual, b"Foreign parent source").unwrap();
        fs::create_dir(claim.parent().unwrap()).unwrap();
        fs::write(claim.parent().unwrap().join("sentinel"), b"Foreign stage").unwrap();
    });
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let result = engine.transfer(
        &transfer::TransferRequest {
            kind: transfer::TransferKind::Move,
            sources: vec![source.clone()],
            destination: target.path().to_owned(),
        },
        &Cancellation::default(),
        |_| transfer::TransferReply::Continue,
    );
    if fs::read(&source).ok().as_deref() != Some(b"Foreign parent source") {
        panic!(
            "source claim used a replacement parent: {result:?}; retained {} and {}",
            root.keep().display(),
            target.keep().display()
        );
    }
    assert_eq!(
        fs::read(parked.join("item")).unwrap(),
        b"Original copied source"
    );
    assert_eq!(
        fs::read(target.path().join("item")).unwrap(),
        b"Original copied source"
    );
}

#[test]
fn a_payload_replaced_at_the_source_unlink_boundary_is_preserved() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("item");
    fs::write(&source, b"Original copied source").unwrap();
    let target = output(root.path());
    let captured = std::rc::Rc::new(RefCell::new(None));
    let at_gate = captured.clone();
    let _reset = install(move |_, payload| {
        fs::rename(payload, payload.with_file_name("original-user-source")).unwrap();
        fs::write(payload, b"Foreign cleanup occupant").unwrap();
        *at_gate.borrow_mut() = Some(payload.to_owned());
    });
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let result = engine.transfer(
        &transfer::TransferRequest {
            kind: transfer::TransferKind::Move,
            sources: vec![source],
            destination: target.path().to_owned(),
        },
        &Cancellation::default(),
        |_| transfer::TransferReply::Continue,
    );
    let payload = captured
        .borrow()
        .clone()
        .expect("real source removal was reached");
    if fs::read(&payload).ok().as_deref() != Some(b"Foreign cleanup occupant") {
        panic!(
            "unlink deleted an unselected source-claim payload: {result:?}; retained {} and {}",
            root.keep().display(),
            target.keep().display()
        );
    }
    assert_eq!(
        fs::read(payload.with_file_name("original-user-source")).unwrap(),
        b"Original copied source"
    );
    assert_eq!(
        fs::read(target.path().join("item")).unwrap(),
        b"Original copied source"
    );
    drop(result);
    drop(engine);
    for _ in 0..2 {
        let reopened = OperationEngine::open(root.path().join("history")).unwrap();
        assert!(
            !reopened
                .interrupted_operations(&Cancellation::default())
                .unwrap()
                .is_empty()
        );
        assert_eq!(fs::read(&payload).unwrap(), b"Foreign cleanup occupant");
        assert_eq!(
            fs::read(payload.with_file_name("original-user-source")).unwrap(),
            b"Original copied source"
        );
    }
}

#[test]
fn killed_source_claims_return_unverified_sources_or_finish_only_verified_removals() {
    use std::os::unix::process::ExitStatusExt;
    for (point, edit_copy) in [
        ("before", false),
        ("ready", true),
        ("claimed", false),
        ("claimed", true),
        ("removed", true),
        ("returning", true),
        ("returned", true),
        ("returned-missing", true),
    ] {
        let root = tempfile::tempdir().unwrap();
        let target = output(root.path());
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::source_removal_boundary::source_claim_crash_helper",
            ])
            .env("CANOPA_SOURCE_CLAIM_ROOT", root.path())
            .env("CANOPA_SOURCE_CLAIM_OUTPUT", target.path())
            .env("CANOPA_SOURCE_CLAIM_POINT", point)
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let payload: PathBuf =
            serde_json::from_slice(&fs::read(root.path().join("claim.json")).unwrap()).unwrap();
        let destination = target.path().join("item");
        let before = fs::metadata(&destination).unwrap();
        if edit_copy {
            fs::write(&destination, b"Newer completed output").unwrap();
        }
        if point == "returned-missing" {
            fs::remove_file(root.path().join("item")).unwrap();
        }
        let preserve_claim = (point == "claimed" && edit_copy) || point == "returned-missing";
        let returned = matches!(point, "before" | "ready" | "returning" | "returned");
        for _ in 0..2 {
            let engine = OperationEngine::open(root.path().join("history")).unwrap();
            assert_eq!(
                !engine
                    .interrupted_operations(&Cancellation::default())
                    .unwrap()
                    .is_empty(),
                preserve_claim,
                "{point}/{edit_copy}"
            );
            assert_eq!(fs::metadata(&destination).unwrap().ino(), before.ino());
            assert_eq!(
                fs::read(&destination).unwrap(),
                if edit_copy {
                    b"Newer completed output".as_slice()
                } else {
                    b"Original copied source".as_slice()
                }
            );
            assert_eq!(
                root.path().join("item").exists(),
                returned,
                "{point}/{edit_copy}"
            );
            if returned {
                assert_eq!(
                    fs::read(root.path().join("item")).unwrap(),
                    b"Original copied source"
                );
            }
            if preserve_claim && point != "returned-missing" {
                assert_eq!(fs::read(&payload).unwrap(), b"Original copied source");
            } else if !preserve_claim {
                assert!(!payload.parent().unwrap().exists());
            }
            let count = engine.state.lock().unwrap().remaining_changes(1).unwrap();
            assert_eq!(count, if returned || preserve_claim { 1 } else { 2 });
        }
    }
}

#[test]
#[ignore = "parent kills the real worker at each durable source-claim boundary"]
fn source_claim_crash_helper() {
    let Some(root) = std::env::var_os("CANOPA_SOURCE_CLAIM_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let target = PathBuf::from(std::env::var_os("CANOPA_SOURCE_CLAIM_OUTPUT").unwrap());
    let point = std::env::var("CANOPA_SOURCE_CLAIM_POINT").unwrap();
    let cancellation = Cancellation::default();
    let _cancel_guard = if point.starts_with("return") {
        let token = cancellation.clone();
        Some(install(move |_, _| token.cancel()))
    } else {
        None
    };
    let source = root.join("item");
    fs::write(&source, b"Original copied source").unwrap();
    let at_gate = root.clone();
    let kill = move |_: &Path, payload: &Path| {
        fs::write(
            at_gate.join("claim.json"),
            serde_json::to_vec(payload).unwrap(),
        )
        .unwrap();
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .unwrap();
        unreachable!();
    };
    let _guard: Box<dyn std::any::Any> = if point == "before" {
        Box::new(rename_boundary::install(kill))
    } else {
        let point = match point.as_str() {
            "ready" => "ready",
            "claimed" => "claimed",
            "removed" => "removed",
            "returning" => "returning",
            "returned" | "returned-missing" => "returned",
            _ => panic!("unknown boundary"),
        };
        AT_PHASE.with(|slot| *slot.borrow_mut() = Some((point, Box::new(kill))));
        Box::new(Reset)
    };
    let engine = OperationEngine::open(root.join("history")).unwrap();
    engine
        .transfer(
            &transfer::TransferRequest {
                kind: transfer::TransferKind::Move,
                sources: vec![source],
                destination: target,
            },
            &cancellation,
            |_| transfer::TransferReply::Continue,
        )
        .unwrap();
    panic!("source-claim boundary was not reached");
}

#[test]
fn restarted_claims_refuse_replacement_copy_generations_even_when_raw_ids_match() {
    use std::os::unix::{fs::PermissionsExt, process::ExitStatusExt};
    for replace_parent in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let target = output(root.path());
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "operations::source_removal_boundary::source_claim_crash_helper",
            ])
            .env("CANOPA_SOURCE_CLAIM_ROOT", root.path())
            .env("CANOPA_SOURCE_CLAIM_OUTPUT", target.path())
            .env("CANOPA_SOURCE_CLAIM_POINT", "claimed")
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(9));
        let payload: PathBuf =
            serde_json::from_slice(&fs::read(root.path().join("claim.json")).unwrap()).unwrap();
        let copy = target.path().join("item");
        let before = fs::metadata(&copy).unwrap();
        let parked = tempfile::Builder::new()
            .prefix(".canopa-source-copy-generation-")
            .tempdir_in(target.path().parent().unwrap())
            .unwrap();
        if replace_parent {
            fs::rename(target.path(), parked.path().join("original-parent")).unwrap();
            fs::create_dir(target.path()).unwrap();
        } else {
            fs::rename(&copy, parked.path().join("original-copy")).unwrap();
        }
        fs::write(&copy, b"Original copied source").unwrap();
        fs::set_permissions(&copy, fs::Permissions::from_mode(before.mode() & 0o777)).unwrap();
        fs::File::open(&copy)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(before.modified().unwrap()))
            .unwrap();
        // Emulate tuple reuse deterministically while preserving the sealed
        // original tokens. Bytes, mode and mtime also match the recorded copy.
        let database = rusqlite::Connection::open(root.path().join("history")).unwrap();
        let body: String = database
            .query_row("SELECT body FROM pending", [], |row| row.get(0))
            .unwrap();
        let mut body: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["direction"], "forward");
        assert_eq!(body["stage"]["source_claim"]["phase"], "claimed");
        let replacement = Identity::read(&copy).unwrap();
        body["change"]["copy_identity"] = serde_json::to_value(replacement).unwrap();
        body["stage"]["source_claim"]["copy_identity"] = serde_json::to_value(replacement).unwrap();
        let expected: Fingerprint =
            serde_json::from_value(body["change"]["fingerprint"].clone()).unwrap();
        assert_eq!(
            Fingerprint::read(&copy, &Cancellation::default()).unwrap(),
            expected
        );
        database
            .execute(
                "UPDATE pending SET body=?1",
                [serde_json::to_string(&body).unwrap()],
            )
            .unwrap();
        drop(database);
        for _ in 0..2 {
            let engine = OperationEngine::open(root.path().join("history")).unwrap();
            assert!(
                !engine
                    .interrupted_operations(&Cancellation::default())
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(fs::read(&payload).unwrap(), b"Original copied source");
            assert!(replacement.matches(&copy));
            assert_eq!(fs::read(&copy).unwrap(), b"Original copied source");
            assert!(!root.path().join("item").exists());
            assert_eq!(
                engine.state.lock().unwrap().remaining_changes(1).unwrap(),
                1
            );
        }
    }
}

/// Overwrite one byte of a file in place, keeping its size and optionally its
/// modification time, as an editor writing through an open file might.
fn edit_in_place(path: &Path, keep_time: bool) {
    use std::io::{Seek, Write};
    let before = fs::metadata(path).unwrap();
    let mut file = fs::File::options().write(true).open(path).unwrap();
    file.seek(io::SeekFrom::Start(8192)).unwrap();
    file.write_all(b"!").unwrap();
    if keep_time {
        file.set_modified(before.modified().unwrap()).unwrap();
    }
    file.sync_all().unwrap();
}

/// On btrfs a move between subvolumes clones its source, and source removal
/// reads shared contents once (ADR 0152). An edit to the claimed source, before
/// either verification, must still return it instead of removing it.
#[test]
fn edited_cloned_sources_are_returned_instead_of_removed() {
    for (point, keep_time) in [
        ("ready", false),
        ("ready", true),
        ("claimed", false),
        ("claimed", true),
    ] {
        let Some(root) = crate::btrfs::test_root() else {
            return;
        };
        crate::btrfs::create_subvolume(root.path(), "from").unwrap();
        crate::btrfs::create_subvolume(root.path(), "to").unwrap();
        let source = root.path().join("from/item");
        let contents: Vec<u8> = (0..1024 * 1024).map(|i| (i % 249) as u8).collect();
        fs::write(&source, &contents).unwrap();
        let edited = std::rc::Rc::new(RefCell::new(false));
        let at_gate = edited.clone();
        AT_PHASE.with(|slot| {
            *slot.borrow_mut() = Some((
                point,
                Box::new(move |_: &Path, payload: &Path| {
                    edit_in_place(payload, keep_time);
                    *at_gate.borrow_mut() = true;
                }),
            ))
        });
        let _reset = Reset;
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        let destination = root.path().join("to");
        let _ = engine.transfer(
            &transfer::TransferRequest {
                kind: transfer::TransferKind::Move,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |_| transfer::TransferReply::Continue,
        );
        assert!(*edited.borrow(), "{point}: source removal was reached");
        let mut expected = contents.clone();
        expected[8192] = b'!';
        assert_eq!(
            fs::read(&source).unwrap(),
            expected,
            "{point}/{keep_time}: the edited source was not returned"
        );
        assert_eq!(fs::read(destination.join("item")).unwrap(), contents);
    }
}

/// The unedited case: the clone is verified once and the source removed.
#[test]
fn cloned_sources_move_between_subvolumes() {
    let Some(root) = crate::btrfs::test_root() else {
        return;
    };
    crate::btrfs::create_subvolume(root.path(), "from").unwrap();
    crate::btrfs::create_subvolume(root.path(), "to").unwrap();
    let source = root.path().join("from/item");
    let contents: Vec<u8> = (0..4 * 1024 * 1024).map(|i| (i % 241) as u8).collect();
    fs::write(&source, &contents).unwrap();
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let destination = root.path().join("to");
    let outcome = engine
        .transfer(
            &transfer::TransferRequest {
                kind: transfer::TransferKind::Move,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &Cancellation::default(),
            |_| transfer::TransferReply::Continue,
        )
        .unwrap();
    assert!(outcome.failures.is_empty(), "{outcome:?}");
    assert!(!source.exists());
    assert_eq!(fs::read(destination.join("item")).unwrap(), contents);
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(fs::read(&source).unwrap(), contents);
    assert!(!destination.join("item").exists());
}
