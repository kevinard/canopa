use canopa_files::{
    Cancellation,
    operations::OperationEngine,
    transfers::{ConflictChoice, TransferEvent, TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::Command,
};

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn chmod(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

// Each child owns its umask. Repair only this test's private directories so a
// failed read-only-folder assertion cannot leave TempDir cleanup incomplete.
fn make_writable(path: &Path) {
    if fs::symlink_metadata(path).is_ok_and(|entry| entry.is_dir()) {
        chmod(path, 0o700);
        for child in fs::read_dir(path).unwrap() {
            make_writable(&child.unwrap().path());
        }
    }
}

struct Cleanup<'a>(&'a Path);
impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        make_writable(self.0);
    }
}

fn fixture(case: &str) {
    let mut failures = Vec::new();
    for mask in ["022", "077"] {
        let root = tempfile::Builder::new()
            .prefix("canopa-directory-permissions-")
            .tempdir()
            .unwrap();
        let _cleanup = Cleanup(root.path());
        let output = Command::new("sh")
            .args([
                "-c",
                "umask \"$1\"; shift; exec \"$@\"",
                "permissions-fixture",
                mask,
            ])
            .arg(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "directory_permissions_fixture",
                "--nocapture",
            ])
            .env("CANOPA_DIRECTORY_PERMISSIONS_ROOT", root.path())
            .env("CANOPA_DIRECTORY_PERMISSIONS_CASE", case)
            .output()
            .unwrap();
        if case.starts_with("crash-") {
            assert_eq!(
                output.status.signal(),
                Some(9),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            after_crash(root.path(), case);
        } else if !output.status.success() {
            failures.push(format!(
                "{case}, umask {mask}:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn copying_private_and_readonly_folders_preserves_modes_and_can_be_undone() {
    fixture("copy");
}

#[test]
fn copying_a_readonly_root_preserves_its_mode_and_can_be_undone() {
    fixture("readonly");
}

#[test]
fn a_private_destination_parent_does_not_broaden_copied_folder_permissions() {
    fixture("private-parent");
}

#[test]
fn continuing_a_cancelled_folder_copy_preserves_modes_and_separate_undo() {
    fixture("continue");
}

#[test]
fn recovery_copies_preserve_private_and_readonly_folder_modes_and_undo() {
    fixture("recovery");
}

#[test]
fn restarting_a_killed_copy_restores_access_before_any_file_commits() {
    fixture("crash-before");
}

#[test]
fn restarting_a_killed_copy_restores_access_and_preserves_its_committed_prefix() {
    fixture("crash-after");
}

#[test]
fn restarting_a_killed_undo_restores_access_and_preserves_newer_contents() {
    fixture("crash-undo");
}

#[test]
fn a_replaced_destination_restores_the_opened_folder_and_can_reconcile_when_it_returns() {
    fixture("replacement");
}

#[test]
fn external_permission_edits_survive_completion_and_kept_folder_undo() {
    fixture("mode-edit");
}

#[test]
fn merging_into_an_unowned_readonly_folder_never_changes_its_permissions() {
    fixture("merge");
}

#[test]
fn continued_readonly_copy_remains_undoable_after_its_original_folder_history_expires() {
    fixture("expiry");
}

#[test]
fn readonly_cross_filesystem_move_preserves_sources_then_retries_and_undoes_original_modes() {
    fixture("cross-move");
}

#[test]
fn undoing_a_cross_filesystem_move_can_remove_its_now_readonly_owned_containers() {
    fixture("cross-output-readonly");
}

#[test]
fn expired_replacements_in_readonly_created_folders_leave_no_empty_stages() {
    fixture("retained-cleanup");
}

#[test]
fn expired_legacy_replacements_never_infer_readonly_parent_authority() {
    fixture("retained-unowned");
}

#[test]
fn expiry_waits_for_the_original_copied_parent_and_preserves_foreign_contents() {
    fixture("retained-parent-replaced");
}

#[test]
fn expiry_preserves_additional_saved_stage_contents_and_allows_copy_out() {
    fixture("retained-extra");
}

#[test]
fn expiry_preserves_edited_saved_bytes_and_allows_copy_out() {
    fixture("retained-edited");
}

#[test]
fn recovery_copy_can_trigger_expiry_in_the_primary_journal_lane() {
    fixture("retained-recovery-lane");
}

#[test]
fn startup_finishes_recorded_expiry_before_payload_unlink() {
    fixture("retained-journal-before");
}

#[test]
fn startup_finishes_recorded_expiry_after_payload_unlink() {
    fixture("retained-journal-unlinked");
}

#[test]
fn startup_finishes_recorded_expiry_after_container_removal() {
    fixture("retained-journal-removed");
}

#[test]
fn recorded_expiry_refuses_a_replaced_stage_until_its_original_returns() {
    fixture("retained-journal-replaced");
}

#[test]
fn recorded_expiry_refuses_a_replaced_parent_until_its_original_returns() {
    fixture("retained-journal-parent");
}

#[test]
fn expiry_follows_the_original_created_parent_through_a_known_rename() {
    fixture("retained-rename");
}

#[test]
fn expiry_follows_saved_data_into_a_cross_filesystem_moves_created_parent() {
    fixture("retained-cross-move");
}

#[test]
fn expiry_follows_saved_data_back_into_a_created_undo_return_parent() {
    fixture("retained-cross-return");
}

#[test]
fn expiry_does_not_adopt_an_existing_cross_filesystem_undo_merge_parent() {
    fixture("retained-cross-merge");
}

#[test]
fn expiry_follows_a_same_filesystem_saved_container_merge_without_adopting_its_parent() {
    fixture("retained-same-merge");
}

#[test]
fn expiry_follows_a_same_filesystem_saved_return_into_a_recreated_parent() {
    fixture("retained-same-return");
}

#[test]
fn prepared_expiry_preserves_saved_bytes_when_an_existing_parent_becomes_readonly() {
    fixture("retained-journal-existing");
}

#[test]
fn expiry_does_not_adopt_an_existing_same_filesystem_undo_merge_parent() {
    fixture("retained-same-undo-merge");
}

#[test]
fn repeated_readonly_copies_keep_the_existing_history_limit() {
    fixture("retention");
}

#[test]
fn startup_preserves_unproved_legacy_folder_preparation_for_review() {
    fixture("install-before");
}

#[test]
fn startup_preserves_unproved_legacy_published_folder_modes_for_review() {
    fixture("install-after");
}

#[test]
fn startup_does_not_adopt_weak_legacy_folder_identity_after_the_original_returns() {
    fixture("install-relocated");
}

#[test]
fn a_new_folder_root_identity_is_not_adopted_from_a_replacement_after_installation() {
    fixture("root-epoch-created");
}

#[test]
fn a_merge_root_identity_is_not_adopted_from_a_replacement_after_approval() {
    fixture("root-epoch-merged");
}

#[test]
fn refusing_placement_after_root_install_keeps_the_count_and_exact_readonly_continuation() {
    fixture("root-check-pause");
}

#[test]
fn an_initial_unreadable_folder_creates_no_empty_output_or_undo() {
    fixture("root-read-error");
}

#[test]
#[ignore = "invoked by parents in isolated processes with their own umask"]
fn directory_permissions_fixture() {
    let root = PathBuf::from(std::env::var_os("CANOPA_DIRECTORY_PERMISSIONS_ROOT").unwrap());
    assert!(root.starts_with(std::env::temp_dir()));
    assert!(
        root.file_name()
            .unwrap()
            .as_encoded_bytes()
            .starts_with(b"canopa-directory-permissions-")
    );
    match std::env::var("CANOPA_DIRECTORY_PERMISSIONS_CASE")
        .unwrap()
        .as_str()
    {
        "copy" => copy(&root, false, false, &[0o700, 0o555, 0o500, 0o750]),
        "readonly" => copy(&root, false, false, &[0o555]),
        "private-parent" => copy(&root, false, true, &[0o700]),
        "continue" => copy(&root, true, false, &[0o700, 0o555, 0o500, 0o750]),
        "recovery" => recovery(&root),
        case if case.starts_with("crash-") => crash(&root, case),
        "replacement" => replacement(&root),
        "mode-edit" => mode_edit(&root),
        "merge" => merge(&root),
        "expiry" => expiry(&root),
        "cross-move" => cross_move(&root),
        "cross-output-readonly" => cross_output_readonly(&root),
        "retention" => retention(&root),
        case if case.starts_with("retained-") => retained_cleanup(&root, case),
        case if case.starts_with("install-") => interrupted_install(&root, case),
        "root-epoch-created" => root_epoch(&root, false),
        "root-epoch-merged" => root_epoch(&root, true),
        "root-check-pause" => root_check_pause(&root),
        "root-read-error" => root_read_error(&root),
        _ => panic!("unknown private fixture case"),
    }
}

fn crash(root: &Path, case: &str) {
    let source = root.join("source");
    tree(&source, 0o555);
    let target = root.join("target");
    fs::create_dir(&target).unwrap();
    let destination = target.join("source");
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let result = engine.transfer(&TransferRequest { kind: TransferKind::Copy, sources: vec![source.clone()], destination: target }, &Cancellation::default(), |event| {
        if case != "crash-undo" && matches!(event, TransferEvent::Progress { ref path, .. } if path == &source.join("a.txt")) && destination.join("a.txt").exists() == (case == "crash-after") {
            assert_eq!(mode(&destination), 0o755);
            rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL).unwrap();
        }
        TransferReply::Continue
    }).unwrap();
    assert!(result.failures.is_empty(), "{result:?}");
    if case == "crash-undo" {
        fs::write(destination.join("a.txt"), b"newer output").unwrap();
        engine
            .undo_interactive(None, &Cancellation::default(), |conflict| {
                assert_eq!(
                    conflict.source,
                    canopa_files::operations::UndoSource::ChangedCopy
                );
                assert_eq!(mode(&destination), 0o755);
                rustix::process::kill_process(
                    rustix::process::getpid(),
                    rustix::process::Signal::KILL,
                )
                .unwrap();
                unreachable!()
            })
            .unwrap();
    }
    panic!("child never reached the intended live-access kill point");
}

fn after_crash(root: &Path, case: &str) {
    use canopa_files::operations::{UndoChoice, UndoSource};
    let destination = root.join("target/source");
    assert_eq!(
        mode(&destination),
        0o755,
        "actual temporary access at process death"
    );
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    assert_eq!(mode(&destination), 0o555, "startup restored read-only mode");
    assert!(
        engine
            .interrupted_operations(&Cancellation::default())
            .unwrap()
            .is_empty()
    );
    if case == "crash-undo" {
        assert_eq!(
            fs::read(destination.join("a.txt")).unwrap(),
            b"newer output"
        );
        engine
            .undo_interactive(None, &Cancellation::default(), |conflict| {
                match conflict.source {
                    UndoSource::ChangedCopy => UndoChoice::KeepItem,
                    UndoSource::NonemptyFolder => UndoChoice::KeepItem,
                    _ => panic!("unexpected review: {conflict:?}"),
                }
            })
            .unwrap();
        assert_eq!(mode(&destination), 0o555);
        assert_eq!(
            fs::read(destination.join("a.txt")).unwrap(),
            b"newer output"
        );
    } else {
        assert_eq!(
            fs::read_dir(&destination).unwrap().count(),
            usize::from(case == "crash-after")
        );
        if case == "crash-after" {
            assert_eq!(
                fs::read(destination.join("a.txt")).unwrap(),
                b"completed prefix"
            );
        }
        engine.undo(&Cancellation::default()).unwrap();
        assert!(!destination.exists());
    }
    check_tree(&root.join("source"), 0o555);
}

fn replacement(root: &Path) {
    use std::cell::Cell;
    let source = root.join("source");
    tree(&source, 0o555);
    let target = root.join("target");
    fs::create_dir(&target).unwrap();
    let destination = target.join("source");
    let held = target.join("original output held");
    let swapped = Cell::new(false);
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let _result = engine.transfer(&TransferRequest { kind: TransferKind::Copy, sources: vec![source.clone()], destination: target.clone() }, &Cancellation::default(), |event| {
        if !swapped.get() && matches!(event, TransferEvent::Progress { ref path, .. } if path == &source.join("a.txt")) {
            fs::rename(&destination, &held).unwrap();
            fs::create_dir(&destination).unwrap(); chmod(&destination, 0o700);
            fs::write(destination.join("foreign.txt"), b"unrelated replacement").unwrap();
            swapped.set(true);
        }
        TransferReply::Continue
    });
    assert!(swapped.get());
    assert_eq!(
        mode(&held),
        0o555,
        "restore the opened original despite its new name"
    );
    assert_eq!(
        mode(&destination),
        0o700,
        "leave the replacement's permissions alone"
    );
    assert_eq!(
        fs::read(destination.join("foreign.txt")).unwrap(),
        b"unrelated replacement"
    );
    assert!(!destination.join("a.txt").exists());
    assert!(
        !engine
            .interrupted_operations(&Cancellation::default())
            .unwrap()
            .is_empty()
    );
    drop(engine);
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    assert_eq!(mode(&held), 0o555);
    assert_eq!(mode(&destination), 0o700);
    assert!(
        !engine
            .interrupted_operations(&Cancellation::default())
            .unwrap()
            .is_empty()
    );
    let foreign = target.join("foreign output held");
    fs::rename(&destination, &foreign).unwrap();
    fs::rename(&held, &destination).unwrap();
    engine.retry_recovery(&Cancellation::default()).unwrap();
    assert_eq!(mode(&destination), 0o555);
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!destination.exists());
    assert_eq!(
        fs::read(foreign.join("foreign.txt")).unwrap(),
        b"unrelated replacement"
    );
    check_tree(&source, 0o555);
}

fn mode_edit(root: &Path) {
    use canopa_files::operations::{UndoChoice, UndoSource};
    use std::cell::Cell;
    let source = root.join("source");
    tree(&source, 0o555);
    let target = root.join("target");
    fs::create_dir(&target).unwrap();
    let destination = target.join("source");
    let edited = Cell::new(false);
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let result = engine.transfer(&TransferRequest { kind: TransferKind::Copy, sources: vec![source.clone()], destination: target }, &Cancellation::default(), |event| {
        if !edited.get() && matches!(event, TransferEvent::Progress { ref path, .. } if path == &source.join("a.txt")) { chmod(&destination, 0o701); edited.set(true); }
        TransferReply::Continue
    }).unwrap();
    assert!(result.failures.is_empty(), "{result:?}");
    check_tree(&destination, 0o701);
    fs::write(destination.join("newer.txt"), b"newer kept work").unwrap();
    engine
        .undo_interactive(None, &Cancellation::default(), |conflict| {
            assert_eq!(conflict.source, UndoSource::NonemptyFolder);
            UndoChoice::KeepItem
        })
        .unwrap();
    assert_eq!(mode(&destination), 0o701);
    assert_eq!(
        fs::read(destination.join("newer.txt")).unwrap(),
        b"newer kept work"
    );
    check_tree(&source, 0o555);
}

fn merge(root: &Path) {
    let source = root.join("source");
    tree(&source, 0o700);
    let target = root.join("target");
    fs::create_dir(&target).unwrap();
    let destination = target.join("source");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("existing.txt"), b"unowned work").unwrap();
    chmod(&destination, 0o555);
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: target,
            },
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Conflict { folders: true, .. }) {
                    TransferReply::Resolve {
                        choice: ConflictChoice::Merge,
                        apply_to_remaining: false,
                    }
                } else {
                    TransferReply::Continue
                }
            },
        )
        .unwrap();
    assert!(!result.failures.is_empty());
    assert_eq!(mode(&destination), 0o555);
    assert_eq!(
        fs::read(destination.join("existing.txt")).unwrap(),
        b"unowned work"
    );
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
    assert!(engine.undo_label().is_none());
    check_tree(&source, 0o700);
}

fn expiry(root: &Path) {
    let source = root.join("source");
    tree(&source, 0o555);
    let target = root.join("target");
    fs::create_dir(&target).unwrap();
    let destination = target.join("source");
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let partial = engine.transfer(&TransferRequest { kind: TransferKind::Copy, sources: vec![source.clone()], destination: target }, &Cancellation::default(), |event| {
        if matches!(event, TransferEvent::Progress { ref path, .. } if path == &source.join("a.txt") && destination.join("a.txt").exists()) { TransferReply::Cancel } else { TransferReply::Continue }
    }).unwrap();
    assert!(partial.cancelled && partial.failures.is_empty());
    assert_eq!(mode(&destination), 0o555);
    for number in 0..100 {
        engine
            .execute(
                canopa_files::operations::OperationRequest::CreateFolder {
                    path: root.join(format!("new-folder-{number}")),
                },
                &Cancellation::default(),
            )
            .unwrap();
    }
    let remaining = engine
        .resume_transfer(
            &partial.continuation.unwrap(),
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(remaining.failures.is_empty(), "{remaining:?}");
    check_tree(&destination, 0o555);
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(mode(&destination), 0o555);
    assert_eq!(
        fs::read(destination.join("a.txt")).unwrap(),
        b"completed prefix"
    );
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
    check_tree(&source, 0o555);
}

fn cross_move(root: &Path) {
    use std::os::unix::fs::MetadataExt;
    let other = tempfile::Builder::new()
        .prefix("canopa-permissions-cross-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let _cleanup = Cleanup(other.path());
    assert_ne!(
        fs::metadata(root).unwrap().dev(),
        fs::metadata(other.path()).unwrap().dev(),
        "test requires two actual filesystems"
    );
    let source = root.join("source");
    tree(&source, 0o555);
    let destination = other.path().join("source");
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let partial = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: other.path().to_owned(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(!partial.failures.is_empty());
    check_tree(&source, 0o555);
    check_tree(&destination, 0o555);
    chmod(&source, 0o700);
    chmod(&source.join("nested"), 0o700);
    let resumed = engine
        .resume_transfer(
            &partial.retry_continuation.unwrap(),
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Conflict { .. }) {
                    TransferReply::Resolve {
                        choice: ConflictChoice::Replace,
                        apply_to_remaining: false,
                    }
                } else {
                    TransferReply::Continue
                }
            },
        )
        .unwrap();
    assert!(resumed.failures.is_empty(), "{resumed:?}");
    assert!(!source.exists());
    check_tree(&destination, 0o555);
    engine.undo(&Cancellation::default()).unwrap();
    check_tree(&source, 0o555);
    check_tree(&destination, 0o555);
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!destination.exists());
    check_tree(&source, 0o555);
}

fn cross_output_readonly(root: &Path) {
    use std::os::unix::fs::MetadataExt;
    let other = tempfile::Builder::new()
        .prefix("canopa-permissions-cross-")
        .tempdir_in(env!("CARGO_MANIFEST_DIR"))
        .unwrap();
    let _cleanup = Cleanup(other.path());
    assert_ne!(
        fs::metadata(root).unwrap().dev(),
        fs::metadata(other.path()).unwrap().dev()
    );
    let source = root.join("source");
    tree(&source, 0o700);
    chmod(&source.join("nested"), 0o700);
    let destination = other.path().join("source");
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let moved = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Move,
                sources: vec![source.clone()],
                destination: other.path().to_owned(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(moved.failures.is_empty(), "{moved:?}");
    assert!(!source.exists());
    chmod(&destination, 0o555);
    chmod(&destination.join("nested"), 0o555);
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(mode(&source), 0o700);
    assert_eq!(mode(&source.join("nested")), 0o700);
    assert_eq!(fs::read(source.join("a.txt")).unwrap(), b"completed prefix");
    assert_eq!(fs::read(source.join("z.txt")).unwrap(), b"remaining file");
    assert_eq!(
        fs::read(source.join("nested/note.txt")).unwrap(),
        b"nested contents"
    );
    assert!(!destination.exists());
    assert!(engine.undo_label().is_none());
}

fn retained_cleanup(root: &Path, case: &str) {
    use canopa_files::operations::OperationRequest;
    use std::io::Write;
    let source = root.join("source");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("a.txt"), vec![b'a'; 256 * 1024]).unwrap();
    fs::write(source.join("b.txt"), b"incoming document").unwrap();
    chmod(&source, 0o555);
    let target = root.join("target");
    fs::create_dir(&target).unwrap();
    let mut destination = target.join("source");
    let history = root.join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let mut arrived = false;
    let copied = engine.transfer(&TransferRequest {
        kind: TransferKind::Copy,
        sources: vec![source.clone()],
        destination: target,
    }, &Cancellation::default(), |event| {
        if matches!(event, TransferEvent::Progress { ref path, .. } if path == &source.join("a.txt")) && !arrived {
            let mut document = fs::OpenOptions::new().write(true).create_new(true).open(destination.join("b.txt")).unwrap();
            document.write_all(b"independent arriving document").unwrap();
            arrived = true;
        }
        if matches!(event, TransferEvent::Conflict { .. }) {
            TransferReply::Resolve { choice: ConflictChoice::Replace, apply_to_remaining: false }
        } else { TransferReply::Continue }
    }).unwrap();
    assert!(
        arrived && !copied.cancelled && copied.failures.is_empty(),
        "{copied:?}"
    );
    assert_eq!(mode(&destination), 0o555);
    let page = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap();
    assert_eq!(page.items.len(), 1);
    let mut previous = page.items[0].entry.as_ref().unwrap().path.clone();
    let mut stage = previous.parent().unwrap().to_owned();
    assert_eq!(
        fs::read(&previous).unwrap(),
        b"independent arriving document"
    );
    drop(engine);
    let engine = OperationEngine::open(&history).unwrap();
    let cross = case.starts_with("retained-cross-").then(|| {
        tempfile::Builder::new()
            .prefix("canopa-expiry-cross-")
            .tempdir_in(env!("CARGO_MANIFEST_DIR"))
            .unwrap()
    });
    let _cross_cleanup = cross.as_ref().map(|directory| Cleanup(directory.path()));
    if case == "retained-rename" {
        let renamed = destination.with_file_name("renamed copied folder");
        engine
            .execute(
                OperationRequest::Rename {
                    source: destination.clone(),
                    destination: renamed.clone(),
                },
                &Cancellation::default(),
            )
            .unwrap();
        destination = renamed;
        stage = destination.join(stage.file_name().unwrap());
        previous = stage.join("previous");
    } else if let Some(cross) = &cross {
        let original_copied = destination.clone();
        use std::os::unix::fs::MetadataExt;
        assert_ne!(
            fs::metadata(root).unwrap().dev(),
            fs::metadata(cross.path()).unwrap().dev()
        );
        chmod(&destination, 0o700);
        let moved = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![destination.clone()],
                    destination: cross.path().to_owned(),
                },
                &Cancellation::default(),
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert!(moved.failures.is_empty() && !moved.cancelled, "{moved:?}");
        assert!(!destination.exists());
        destination = cross.path().join("source");
        stage = destination.join(stage.file_name().unwrap());
        previous = stage.join("previous");
        assert_eq!(
            fs::read(&previous).unwrap(),
            b"independent arriving document"
        );
        if matches!(case, "retained-cross-return" | "retained-cross-merge") {
            if case == "retained-cross-merge" {
                fs::create_dir(&original_copied).unwrap();
                fs::write(
                    original_copied.join("foreign.txt"),
                    b"existing merge folder work",
                )
                .unwrap();
            }
            let undone = engine
                .undo_interactive(None, &Cancellation::default(), |_| {
                    canopa_files::operations::UndoChoice::Merge
                })
                .unwrap();
            assert!(!undone.cancelled, "{undone:?}");
            assert!(!destination.exists());
            destination = original_copied;
            stage = destination.join(stage.file_name().unwrap());
            previous = stage.join("previous");
            assert_eq!(
                fs::read(&previous).unwrap(),
                b"independent arriving document"
            );
        }
        chmod(&destination, 0o555);
    } else if matches!(
        case,
        "retained-same-merge" | "retained-same-return" | "retained-same-undo-merge"
    ) {
        let original_copied = destination.clone();
        let parent = root.join("existing merge target");
        fs::create_dir(&parent).unwrap();
        let merged = parent.join("source");
        fs::create_dir(&merged).unwrap();
        fs::write(merged.join("foreign.txt"), b"existing merge folder work").unwrap();
        chmod(&destination, 0o700);
        let moved = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Move,
                    sources: vec![destination.clone()],
                    destination: parent,
                },
                &Cancellation::default(),
                |event| {
                    if matches!(event, TransferEvent::Conflict { folders: true, .. }) {
                        TransferReply::Resolve {
                            choice: ConflictChoice::Merge,
                            apply_to_remaining: false,
                        }
                    } else {
                        TransferReply::Continue
                    }
                },
            )
            .unwrap();
        assert!(moved.failures.is_empty() && !moved.cancelled, "{moved:?}");
        assert!(!destination.exists());
        destination = merged;
        stage = destination.join(stage.file_name().unwrap());
        previous = stage.join("previous");
        assert_eq!(
            fs::read(&previous).unwrap(),
            b"independent arriving document"
        );
        if matches!(case, "retained-same-return" | "retained-same-undo-merge") {
            if case == "retained-same-undo-merge" {
                fs::create_dir(&original_copied).unwrap();
                fs::write(
                    original_copied.join("foreign.txt"),
                    b"existing merge folder work",
                )
                .unwrap();
                engine
                    .undo_interactive(None, &Cancellation::default(), |_| {
                        canopa_files::operations::UndoChoice::Merge
                    })
                    .unwrap();
            } else {
                engine.undo(&Cancellation::default()).unwrap();
            }
            assert_eq!(
                fs::read(destination.join("foreign.txt")).unwrap(),
                b"existing merge folder work"
            );
            destination = original_copied;
            stage = destination.join(stage.file_name().unwrap());
            previous = stage.join("previous");
            assert_eq!(
                fs::read(&previous).unwrap(),
                b"independent arriving document"
            );
        }
        chmod(&destination, 0o555);
    }
    let extra = stage.join("unexpected.txt");
    let held = destination.with_file_name("held original copied folder");
    if case == "retained-unowned" {
        // Legacy input has no permission-changing authority. An independently
        // created Merge target is covered separately by the ordinary tests.
        drop(engine);
        let connection = rusqlite::Connection::open(&history).unwrap();
        let rows = connection
            .prepare("SELECT operation,ordinal,body FROM changes")
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, u64>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for (operation, ordinal, body) in rows {
            let mut change: serde_json::Value = serde_json::from_str(&body).unwrap();
            change.as_object_mut().unwrap().remove("access_parent");
            if let Some(stage) = change
                .get_mut("retained")
                .and_then(|retained| retained.get_mut("stage"))
                .and_then(serde_json::Value::as_object_mut)
            {
                stage.remove("parent_access");
            }

            connection
                .execute(
                    "UPDATE changes SET body=?3 WHERE operation=?1 AND ordinal=?2",
                    rusqlite::params![operation, ordinal, change.to_string()],
                )
                .unwrap();
            connection
                .execute(
                    "DELETE FROM identity_refs WHERE operation=?1 AND ordinal=?2 AND role=1",
                    rusqlite::params![operation, ordinal],
                )
                .unwrap();
        }
        connection.pragma_update(None, "user_version", 18).unwrap();
        drop(connection);
    } else {
        drop(engine);
    }
    let engine = OperationEngine::open(&history).unwrap();
    if case == "retained-parent-replaced" {
        fs::rename(&destination, &held).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("foreign.txt"), b"unrelated replacement").unwrap();
        chmod(&destination, 0o700);
    } else if case == "retained-extra"
        || case.starts_with("retained-journal-")
        || case == "retained-recovery-lane"
    {
        fs::write(&extra, b"newer stage work").unwrap();
    } else if case == "retained-edited" {
        fs::write(&previous, b"newer saved document").unwrap();
    }
    for number in 0..100 {
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.join(format!("history-{number}")),
                },
                &Cancellation::default(),
            )
            .unwrap();
    }
    let page = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap();
    if matches!(
        case,
        "retained-unowned"
            | "retained-cross-merge"
            | "retained-same-merge"
            | "retained-same-undo-merge"
    ) {
        if case != "retained-unowned" {
            assert_eq!(
                fs::read(destination.join("foreign.txt")).unwrap(),
                b"existing merge folder work"
            );
        }
        assert!(
            stage.exists(),
            "an existing Merge parent must not gain automatic permission authority"
        );
        assert_eq!(mode(&destination), 0o555);
        assert_eq!(
            fs::read(&previous).unwrap(),
            b"independent arriving document"
        );
        assert!(
            page.interrupted.is_empty() && page.items.len() == 1,
            "{page:?}"
        );
        chmod(&destination, 0o700);
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.join("permission repaired"),
                },
                &Cancellation::default(),
            )
            .unwrap();
        chmod(&destination, 0o555);
    } else if case == "retained-parent-replaced" {
        assert_eq!(mode(&destination), 0o700);
        assert_eq!(
            fs::read(destination.join("foreign.txt")).unwrap(),
            b"unrelated replacement"
        );
        assert_eq!(
            fs::read(held.join(previous.strip_prefix(&destination).unwrap())).unwrap(),
            b"independent arriving document"
        );
        assert!(
            page.interrupted.is_empty() && page.items.len() == 1,
            "{page:?}"
        );
        let foreign = root.join("foreign folder held");
        fs::rename(&destination, &foreign).unwrap();
        fs::rename(&held, &destination).unwrap();
        engine
            .execute(
                OperationRequest::CreateFolder {
                    path: root.join("original returned"),
                },
                &Cancellation::default(),
            )
            .unwrap();
        assert_eq!(mode(&foreign), 0o700);
        assert_eq!(
            fs::read(foreign.join("foreign.txt")).unwrap(),
            b"unrelated replacement"
        );
    } else if case == "retained-extra" || case == "retained-edited" {
        assert_eq!(mode(&destination), 0o555);
        assert!(
            stage.is_dir() && page.interrupted.is_empty() && page.items.len() == 1,
            "{page:?}"
        );
        assert_eq!(
            fs::read(&previous).unwrap(),
            if case == "retained-edited" {
                b"newer saved document".as_slice()
            } else {
                b"independent arriving document".as_slice()
            }
        );
        if case == "retained-extra" {
            assert_eq!(fs::read(&extra).unwrap(), b"newer stage work");
        }
        let recovered = root.join("recovered saved document.txt");
        engine
            .recover_copy(&page.items[0], &recovered, &Cancellation::default())
            .unwrap();
        assert_eq!(fs::read(&recovered).unwrap(), fs::read(&previous).unwrap());
        assert_eq!(
            fs::read(source.join("b.txt")).unwrap(),
            b"incoming document"
        );
        return;
    } else if case == "retained-recovery-lane" {
        let selected = page.items[0].clone();
        fs::remove_file(&extra).unwrap();
        let recovered = root.join("saved copy after expiry.txt");
        engine
            .recover_copy(&selected, &recovered, &Cancellation::default())
            .unwrap();
        assert_eq!(
            fs::read(&recovered).unwrap(),
            b"independent arriving document"
        );
    } else if case.starts_with("retained-journal-") {
        drop(engine);
        fs::remove_file(&extra).unwrap();
        let connection = rusqlite::Connection::open(&history).unwrap();
        let (entry, body): (u64, String) = connection
            .query_row("SELECT id,body FROM garbage", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        let mut record: serde_json::Value = serde_json::from_str(&body).unwrap();
        if case == "retained-journal-existing" {
            record["retained"]["stage"]["parent_access"] = serde_json::json!({"kind":"existing"});
            connection
                .execute(
                    "UPDATE garbage SET body=?2 WHERE id=?1",
                    rusqlite::params![entry, record.to_string()],
                )
                .unwrap();
        }
        let intent = serde_json::json!({"direction":"expiry", "entry":entry, "parent":record["retained"]["stage"]["parent_identity"], "record":record});
        connection
            .execute(
                "INSERT INTO pending(singleton,body) VALUES(1,?1)",
                [intent.to_string()],
            )
            .unwrap();
        drop(connection);
        if matches!(
            case,
            "retained-journal-unlinked" | "retained-journal-removed"
        ) {
            fs::remove_file(&previous).unwrap();
        }
        if case == "retained-journal-removed" {
            chmod(&destination, 0o700);
            fs::remove_dir(&stage).unwrap();
            chmod(&destination, 0o555);
        }
        let held_stage = root.join("held original stage");
        if case == "retained-journal-replaced" {
            chmod(&destination, 0o700);
            fs::rename(&stage, &held_stage).unwrap();
            fs::create_dir(&stage).unwrap();
            fs::write(stage.join("foreign.txt"), b"foreign stage work").unwrap();
            chmod(&destination, 0o555);
        }
        if case == "retained-journal-parent" {
            fs::rename(&destination, &held).unwrap();
            fs::create_dir(&destination).unwrap();
            fs::write(destination.join("foreign.txt"), b"foreign parent work").unwrap();
            chmod(&destination, 0o700);
        }
        let engine = OperationEngine::open(&history).unwrap();
        if case == "retained-journal-existing" {
            assert!(
                !engine
                    .interrupted_operations(&Cancellation::default())
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(mode(&destination), 0o555);
            assert_eq!(
                fs::read(&previous).unwrap(),
                b"independent arriving document"
            );
            let page = engine
                .recovery_items(None, &Cancellation::default())
                .unwrap();
            assert_eq!(page.items.len(), 1);
            assert!(page.items[0].entry.is_some());
            chmod(&destination, 0o700);
            engine.retry_recovery(&Cancellation::default()).unwrap();
            assert_eq!(
                mode(&destination),
                0o700,
                "explicit permission repair remains a user edit"
            );
            chmod(&destination, 0o555);
        } else if case == "retained-journal-replaced" {
            assert!(
                !engine
                    .interrupted_operations(&Cancellation::default())
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(mode(&destination), 0o555);
            assert_eq!(
                fs::read(stage.join("foreign.txt")).unwrap(),
                b"foreign stage work"
            );
            assert_eq!(
                fs::read(held_stage.join("previous")).unwrap(),
                b"independent arriving document"
            );
            let foreign_stage = root.join("foreign stage held");
            chmod(&destination, 0o700);
            fs::rename(&stage, &foreign_stage).unwrap();
            fs::rename(&held_stage, &stage).unwrap();
            chmod(&destination, 0o555);
            engine.retry_recovery(&Cancellation::default()).unwrap();
            assert_eq!(
                fs::read(foreign_stage.join("foreign.txt")).unwrap(),
                b"foreign stage work"
            );
        } else if case == "retained-journal-parent" {
            assert!(
                !engine
                    .interrupted_operations(&Cancellation::default())
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(mode(&destination), 0o700);
            assert_eq!(
                fs::read(destination.join("foreign.txt")).unwrap(),
                b"foreign parent work"
            );
            assert_eq!(mode(&held), 0o555);
            let foreign = root.join("foreign parent held");
            fs::rename(&destination, &foreign).unwrap();
            fs::rename(&held, &destination).unwrap();
            engine.retry_recovery(&Cancellation::default()).unwrap();
            assert_eq!(
                fs::read(foreign.join("foreign.txt")).unwrap(),
                b"foreign parent work"
            );
        }
        assert!(
            engine
                .interrupted_operations(&Cancellation::default())
                .unwrap()
                .is_empty()
        );
        assert!(
            engine
                .recovery_items(None, &Cancellation::default())
                .unwrap()
                .items
                .is_empty()
        );
        assert!(!stage.exists());
        assert_eq!(mode(&destination), 0o555);
        assert_eq!(
            fs::read(destination.join("b.txt")).unwrap(),
            b"incoming document"
        );
        assert_eq!(
            fs::read(source.join("b.txt")).unwrap(),
            b"incoming document"
        );
        return;
    }
    let page = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap();
    assert!(
        !stage.exists(),
        "expired replacement left a private stage: {stage:?}; public Recovery: {page:?}"
    );
    assert!(
        page.items.is_empty() && page.interrupted.is_empty(),
        "{page:?}"
    );
    assert_eq!(mode(&destination), 0o555);
    assert_eq!(
        fs::read(destination.join("b.txt")).unwrap(),
        b"incoming document"
    );
    assert_eq!(
        fs::read(source.join("b.txt")).unwrap(),
        b"incoming document"
    );
}

fn retention(root: &Path) {
    let source = root.join("source");
    fs::create_dir(&source).unwrap();
    fs::write(source.join("note.txt"), b"retained copied contents").unwrap();
    chmod(&source, 0o555);
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let mut operation_ids = std::collections::BTreeSet::new();
    for number in 0..101 {
        let target = root.join(format!("output-{number}"));
        fs::create_dir(&target).unwrap();
        let result = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![source.clone()],
                    destination: target,
                },
                &Cancellation::default(),
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert!(result.failures.is_empty(), "{result:?}");
        assert!(operation_ids.insert(result.id.unwrap()));
        assert!(
            engine
                .interrupted_operations(&Cancellation::default())
                .unwrap()
                .is_empty()
        );
    }
    for _ in 0..100 {
        engine.undo(&Cancellation::default()).unwrap();
    }
    assert!(
        engine.undo_label().is_none(),
        "temporary folder access must not suppress normal history expiry"
    );
    assert_eq!(mode(&root.join("output-0/source")), 0o555);
    assert_eq!(
        fs::read(root.join("output-0/source/note.txt")).unwrap(),
        b"retained copied contents"
    );
    for number in 1..101 {
        assert!(!root.join(format!("output-{number}/source")).exists());
    }
    assert_eq!(mode(&source), 0o555);
    assert_eq!(
        fs::read(source.join("note.txt")).unwrap(),
        b"retained copied contents"
    );
}

fn interrupted_install(root: &Path, phase: &str) {
    use serde_json::json;
    use std::os::unix::fs::MetadataExt;
    let identity = |path: &Path| {
        let m = fs::symlink_metadata(path).unwrap();
        json!({"device":m.dev(),"inode":m.ino()})
    };
    let path = |value: &Path| json!({"unix_bytes":value.as_os_str().as_encoded_bytes()});
    let source = root.join("source");
    tree(&source, 0o555);
    let target = root.join("target");
    fs::create_dir(&target).unwrap();
    let destination = target.join("output");
    let history = root.join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let seed = engine
        .execute(
            canopa_files::operations::OperationRequest::CreateFolder {
                path: root.join("history anchor"),
            },
            &Cancellation::default(),
        )
        .unwrap();
    drop(engine);
    let stage = target.join(".canopa-files-stage-mode-input");
    fs::create_dir(&stage).unwrap();
    chmod(&stage, 0o700);
    let payload = stage.join("folder");
    fs::create_dir(&payload).unwrap();
    chmod(&payload, 0o755);
    let item_identity = identity(&payload);
    let intent = json!({"direction":"forward","id":seed.id,"ordinal":1,"change":{"kind":"created-folder","path":path(&destination),"identity":item_identity,"mode":0o555},"stage":{"directory":path(&stage),"identity":identity(&stage),"parent_identity":identity(&target),"payload_identity":item_identity}});
    if phase != "install-before" {
        fs::rename(&payload, &destination).unwrap();
    }
    let held = target.join("original installed folder held");
    if phase == "install-relocated" {
        fs::rename(&destination, &held).unwrap();
        fs::create_dir(&destination).unwrap();
        chmod(&destination, 0o700);
        fs::write(destination.join("foreign.txt"), b"unrelated occupant").unwrap();
    }
    // Input preparation only: emulate death with a committed journal before or
    // after its kernel rename. Observe recovery through Engine and real files.
    rusqlite::Connection::open(&history)
        .unwrap()
        .execute(
            "INSERT INTO pending(singleton,body) VALUES(1,?1)",
            [intent.to_string()],
        )
        .unwrap();
    let engine = OperationEngine::open(&history).unwrap();
    if phase == "install-relocated" {
        assert!(
            !engine
                .interrupted_operations(&Cancellation::default())
                .unwrap()
                .is_empty()
        );
        assert_eq!(mode(&destination), 0o700);
        assert_eq!(
            fs::read(destination.join("foreign.txt")).unwrap(),
            b"unrelated occupant"
        );
        let foreign = target.join("foreign occupant held");
        fs::rename(&destination, &foreign).unwrap();
        fs::rename(&held, &destination).unwrap();
        assert!(engine.retry_recovery(&Cancellation::default()).is_err());
        assert_eq!(
            fs::read(foreign.join("foreign.txt")).unwrap(),
            b"unrelated occupant"
        );
    }
    // This hand-written legacy record has only inode hints, not an original
    // payload/parent proof or bound Forward context. Returning the same inode
    // must not manufacture authority to chmod, publish or delete its folder.
    assert!(
        !engine
            .interrupted_operations(&Cancellation::default())
            .unwrap()
            .is_empty()
    );
    assert!(stage.exists());
    if phase == "install-before" {
        assert!(!destination.exists());
        assert_eq!(mode(&payload), 0o755);
    } else {
        assert_eq!(mode(&destination), 0o755);
    }
    assert!(engine.retry_recovery(&Cancellation::default()).is_err());
    assert!(engine.undo(&Cancellation::default()).is_err());
    assert!(root.join("history anchor").exists());
    assert_eq!(
        rusqlite::Connection::open(&history)
            .unwrap()
            .query_row("SELECT body FROM pending", [], |r| r.get::<_, String>(0))
            .unwrap(),
        intent.to_string()
    );
    check_tree(&source, 0o555);
}

fn root_epoch(root: &Path, merge: bool) {
    use canopa_files::transfers::TransferAccess;
    use std::cell::Cell;
    let modes: &[u32] = if merge { &[0o700] } else { &[0o555, 0o700] };
    for &foreign_mode in modes {
        let base = root.join(format!("mode-{foreign_mode:o}"));
        fs::create_dir(&base).unwrap();
        let source = base.join("source");
        tree(&source, 0o555);
        let target = base.join("target");
        fs::create_dir(&target).unwrap();
        let destination = target.join("source");
        if merge {
            fs::create_dir(&destination).unwrap();
            chmod(&destination, 0o700);
            fs::write(destination.join("approved.txt"), b"approved existing work").unwrap();
        }
        let held = target.join("original root held");
        let swapped = Cell::new(false);
        let engine = OperationEngine::open(base.join("state/history")).unwrap();
        let failed = engine
            .transfer_with_check(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![source.clone()],
                    destination: target.clone(),
                },
                &Cancellation::default(),
                |path, access| {
                    if access == TransferAccess::DestinationFolder
                        && path == destination.as_path()
                        && destination.is_dir()
                        && !swapped.get()
                    {
                        fs::rename(&destination, &held).unwrap();
                        fs::create_dir(&destination).unwrap();
                        fs::write(destination.join("foreign.txt"), b"unselected replacement")
                            .unwrap();
                        chmod(&destination, foreign_mode);
                        swapped.set(true);
                    }
                    Ok(())
                },
                |event| {
                    if matches!(event, TransferEvent::Conflict { folders: true, .. }) {
                        TransferReply::Resolve {
                            choice: ConflictChoice::Merge,
                            apply_to_remaining: false,
                        }
                    } else {
                        TransferReply::Continue
                    }
                },
            )
            .unwrap();
        assert!(
            swapped.get(),
            "exercise the actual root install/approval placement check"
        );
        assert_eq!(mode(&destination), foreign_mode);
        assert_eq!(
            fs::read(destination.join("foreign.txt")).unwrap(),
            b"unselected replacement"
        );
        assert_eq!(
            fs::read_dir(&destination).unwrap().count(),
            1,
            "never copy into a root that only appeared at the same pathname"
        );
        assert!(failed.needs_attention || !failed.failures.is_empty());
        let foreign = target.join("foreign root held");
        fs::rename(&destination, &foreign).unwrap();
        fs::rename(&held, &destination).unwrap();
        let resumed = engine
            .resume_transfer(
                &failed.retry_continuation.unwrap(),
                &Cancellation::default(),
                |event| {
                    assert!(
                        !matches!(event, TransferEvent::Conflict { .. }),
                        "the captured original root already has its folder decision"
                    );
                    TransferReply::Continue
                },
            )
            .unwrap();
        assert!(resumed.failures.is_empty(), "{resumed:?}");
        assert_eq!(mode(&destination), if merge { 0o700 } else { 0o555 });
        assert_eq!(
            fs::read(destination.join("a.txt")).unwrap(),
            b"completed prefix"
        );
        assert_eq!(
            fs::read(destination.join("nested/note.txt")).unwrap(),
            b"nested contents"
        );
        engine.undo(&Cancellation::default()).unwrap();
        if merge {
            assert_eq!(fs::read_dir(&destination).unwrap().count(), 1);
            assert_eq!(
                fs::read(destination.join("approved.txt")).unwrap(),
                b"approved existing work"
            );
        } else {
            assert_eq!(mode(&destination), 0o555);
            assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
            engine.undo(&Cancellation::default()).unwrap();
            assert!(!destination.exists());
        }
        assert_eq!(mode(&foreign), foreign_mode);
        assert_eq!(fs::read_dir(&foreign).unwrap().count(), 1);
        check_tree(&source, 0o555);
    }
}

fn root_check_pause(root: &Path) {
    use canopa_files::transfers::TransferAccess;
    use std::cell::Cell;
    let source = root.join("source");
    tree(&source, 0o555);
    let target = root.join("target");
    fs::create_dir(&target).unwrap();
    let destination = target.join("source");
    let refused = Cell::new(false);
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let partial = engine
        .transfer_with_check(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: target,
            },
            &Cancellation::default(),
            |path, access| {
                if path == destination.as_path()
                    && access == TransferAccess::DestinationFolder
                    && !refused.get()
                {
                    refused.set(true);
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotConnected,
                        "temporary placement refusal after root install",
                    ));
                }
                Ok(())
            },
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(refused.get() && partial.needs_attention && partial.failures.is_empty());
    assert_eq!(
        partial.completed, 1,
        "the installed folder is actual completed work"
    );
    assert!(partial.id.is_some());
    assert_eq!(mode(&destination), 0o555);
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
    let resumed = engine
        .resume_transfer(
            &partial.continuation.unwrap(),
            &Cancellation::default(),
            |event| {
                assert!(
                    !matches!(event, TransferEvent::Conflict { .. }),
                    "Continue must reuse its captured created root"
                );
                TransferReply::Continue
            },
        )
        .unwrap();
    assert!(resumed.failures.is_empty(), "{resumed:?}");
    assert_eq!(resumed.completed, 4, "do not count the original root twice");
    check_tree(&destination, 0o555);
    engine.undo(&Cancellation::default()).unwrap();
    assert_eq!(mode(&destination), 0o555);
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!destination.exists());
    check_tree(&source, 0o555);
}

fn root_read_error(root: &Path) {
    let source = root.join("source");
    tree(&source, 0o555);
    chmod(&source, 0o0);
    let target = root.join("target");
    fs::create_dir(&target).unwrap();
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    let failed = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: target.clone(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(!failed.failures.is_empty());
    assert_eq!(mode(&source), 0o0);
    assert_eq!(failed.completed, 0);
    assert!(failed.id.is_none() && engine.undo_label().is_none());
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    chmod(&source, 0o555);
    check_tree(&source, 0o555);
}

fn tree(path: &Path, permissions: u32) {
    fs::create_dir(path).unwrap();
    fs::write(path.join("a.txt"), b"completed prefix").unwrap();
    fs::write(path.join("z.txt"), b"remaining file").unwrap();
    fs::create_dir(path.join("nested")).unwrap();
    fs::write(path.join("nested/note.txt"), b"nested contents").unwrap();
    chmod(&path.join("nested"), 0o555);
    chmod(path, permissions);
}

fn check_tree(path: &Path, permissions: u32) {
    assert_eq!(mode(path), permissions, "folder mode at {}", path.display());
    assert_eq!(mode(&path.join("nested")), 0o555, "nested read-only mode");
    assert_eq!(fs::read(path.join("a.txt")).unwrap(), b"completed prefix");
    assert_eq!(fs::read(path.join("z.txt")).unwrap(), b"remaining file");
    assert_eq!(
        fs::read(path.join("nested/note.txt")).unwrap(),
        b"nested contents"
    );
}

fn copy(root: &Path, partial: bool, private_parent: bool, modes: &[u32]) {
    for &permissions in modes {
        use std::os::unix::ffi::OsStringExt;
        let mut name = format!("source-{permissions:o}-").into_bytes();
        name.push(0xff);
        let source = root.join(std::ffi::OsString::from_vec(name));
        tree(&source, permissions);
        let target = root.join(format!("target-{permissions:o}"));
        fs::create_dir(&target).unwrap();
        if private_parent {
            chmod(&target, 0o700);
        }
        let destination = target.join(source.file_name().unwrap());
        let engine =
            OperationEngine::open(root.join(format!("state-{permissions:o}/history"))).unwrap();
        let outcome = engine.transfer(&TransferRequest {
            kind: TransferKind::Copy,
            sources: vec![source.clone()],
            destination: target,
        }, &Cancellation::default(), |event| {
            if partial && matches!(event, TransferEvent::Progress { ref path, .. } if path == &source.join("a.txt") && destination.join("a.txt").exists()) {
                TransferReply::Cancel
            } else {
                TransferReply::Continue
            }
        }).unwrap();
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
        assert_eq!(mode(&destination), permissions, "copied folder mode");
        if partial {
            assert!(outcome.cancelled);
            let resumed = engine
                .resume_transfer(
                    &outcome.continuation.unwrap(),
                    &Cancellation::default(),
                    |_| TransferReply::Continue,
                )
                .unwrap();
            assert!(
                resumed.failures.is_empty() && !resumed.cancelled,
                "{resumed:?}"
            );
            check_tree(&destination, permissions);
            engine.undo(&Cancellation::default()).unwrap();
            assert_eq!(mode(&destination), permissions, "mode after partial Undo");
            assert_eq!(
                fs::read(destination.join("a.txt")).unwrap(),
                b"completed prefix"
            );
            assert!(!destination.join("z.txt").exists());
        } else {
            assert!(!outcome.cancelled);
            check_tree(&destination, permissions);
        }
        engine.undo(&Cancellation::default()).unwrap();
        assert!(!destination.exists());
        check_tree(&source, permissions);
    }
}

fn recovery(root: &Path) {
    let source = root.join("incoming.txt");
    fs::write(&source, b"incoming version").unwrap();
    let target = root.join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("incoming.txt"), b"saved version").unwrap();
    let engine = OperationEngine::open(root.join("state/history")).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
                destination: target.clone(),
            },
            &Cancellation::default(),
            |_| TransferReply::Resolve {
                choice: ConflictChoice::Replace,
                apply_to_remaining: false,
            },
        )
        .unwrap();
    let saved = engine
        .recovery_items(None, &Cancellation::default())
        .unwrap()
        .items[0]
        .entry
        .as_ref()
        .unwrap()
        .path
        .clone();
    let held = root.join("held previous file");
    fs::rename(&saved, &held).unwrap();
    tree(&saved, 0o700);
    for permissions in [0o700, 0o555, 0o500, 0o750] {
        chmod(&saved, permissions);
        let item = engine
            .recovery_items(None, &Cancellation::default())
            .unwrap()
            .items
            .remove(0);
        let destination = root.join(format!("recovered-{permissions:o}"));
        let result = engine
            .recover_copy_with_check(
                &item,
                &destination,
                &Cancellation::default(),
                |_, _| Ok(()),
                |_| TransferReply::Continue,
            )
            .unwrap();
        assert!(result.failures.is_empty(), "{:?}", result.failures);
        check_tree(&destination, permissions);
        engine.undo(&Cancellation::default()).unwrap();
        assert!(!destination.exists());
        check_tree(&saved, permissions);
        assert_eq!(fs::read(&held).unwrap(), b"saved version");
        assert_eq!(
            fs::read(target.join("incoming.txt")).unwrap(),
            b"incoming version"
        );
    }
}
