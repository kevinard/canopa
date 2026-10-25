use canopa_files::{
    Cancellation,
    operations::{OperationEngine, requests::RequestPhase},
    transfers::{TransferContinuation, TransferKind, TransferReply, TransferRequest},
};
use std::{
    ffi::OsString,
    fs,
    os::unix::{ffi::OsStringExt, fs::MetadataExt, fs::PermissionsExt},
};

#[test]
fn a_stored_failed_directory_plan_keeps_raw_names_owned_modes_and_separate_undo() {
    let root = tempfile::tempdir().unwrap();
    let source = root
        .path()
        .join(OsString::from_vec(b"project-\xff".to_vec()));
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();
    fs::create_dir(&destination).unwrap();
    let completed = source.join("a-completed.txt");
    let failed = source.join(OsString::from_vec(b"z-retry-\xfe.txt".to_vec()));
    fs::write(&completed, b"completed before restart").unwrap();
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &failed,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let accepted = engine.queue_request(&"directory Copy").unwrap();
    let running = engine
        .checkpoint_request(&accepted, RequestPhase::Running, &"directory Copy")
        .unwrap();
    let outcome = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(outcome.failures.len(), 1);
    let output = destination.join(source.file_name().unwrap());
    let previous = output.join(completed.file_name().unwrap());
    let previous_inode = fs::metadata(&previous).unwrap().ino();
    assert_eq!(fs::read(&previous).unwrap(), b"completed before restart");
    engine
        .checkpoint_request(
            &running,
            RequestPhase::Failed,
            &outcome.retry_continuation.unwrap(),
        )
        .unwrap();
    drop(engine);
    fs::remove_file(&failed).unwrap();
    fs::write(&failed, b"repaired after restart").unwrap();

    let engine = OperationEngine::open(&history).unwrap();
    let stored = engine.request_page(None).unwrap().remove(0);
    assert_eq!(stored.phase(), RequestPhase::Failed);
    let restored: TransferContinuation = stored.payload().unwrap();
    let running = engine
        .checkpoint_request(&stored, RequestPhase::Running, &restored)
        .unwrap();
    let retried = engine
        .resume_transfer(&restored, &cancel, |_| TransferReply::Continue)
        .unwrap();
    assert!(retried.failures.is_empty());
    engine
        .checkpoint_request(&running, RequestPhase::Finished, &"completed exact Retry")
        .unwrap();
    let repaired = output.join(failed.file_name().unwrap());
    assert_eq!(fs::read(&repaired).unwrap(), b"repaired after restart");
    assert_eq!(fs::metadata(&previous).unwrap().ino(), previous_inode);
    assert_eq!(fs::read(&previous).unwrap(), b"completed before restart");
    assert_eq!(fs::metadata(&output).unwrap().mode() & 0o7777, 0o700);
    engine.undo(&cancel).unwrap();
    assert!(!repaired.exists());
    assert_eq!(fs::read(&previous).unwrap(), b"completed before restart");
    engine.undo(&cancel).unwrap();
    assert!(!output.exists());
    assert_eq!(fs::read(completed).unwrap(), b"completed before restart");
    assert_eq!(fs::read(failed).unwrap(), b"repaired after restart");
}

#[test]
fn a_decoded_plan_refuses_a_replaced_source_folder_until_the_original_returns() {
    replaced_folder(true);
}

#[test]
fn a_decoded_plan_refuses_a_replaced_destination_folder_until_the_original_returns() {
    replaced_folder(false);
}

fn replaced_folder(source_replaced: bool) {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("project");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("a-completed"), b"older completed bytes").unwrap();
    let failed = source.join("z-retry");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &failed,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let history = root.path().join("state/history");
    let engine = OperationEngine::open(&history).unwrap();
    let cancel = Cancellation::default();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: destination.clone(),
            },
            &cancel,
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(result.failures.len(), 1);
    let stored = serde_json::to_vec(&result.retry_continuation.unwrap()).unwrap();
    drop(engine);
    fs::remove_file(&failed).unwrap();
    fs::write(&failed, b"the repaired original").unwrap();
    let output = destination.join("project");
    let replaced = if source_replaced { &source } else { &output };
    let held = root.path().join("held-original");
    fs::rename(replaced, &held).unwrap();
    fs::create_dir(replaced).unwrap();
    fs::write(replaced.join("z-retry"), b"independent foreign bytes").unwrap();
    let restored: TransferContinuation = serde_json::from_slice(&stored).unwrap();
    let engine = OperationEngine::open(&history).unwrap();
    let refused = engine
        .resume_transfer(&restored, &cancel, |_| TransferReply::Continue)
        .unwrap();
    assert_eq!(refused.completed, 0);
    assert!(!refused.failures.is_empty());
    assert_eq!(
        fs::read(replaced.join("z-retry")).unwrap(),
        b"independent foreign bytes"
    );
    if source_replaced {
        assert!(!output.join("z-retry").exists());
    } else {
        assert_eq!(fs::read(&failed).unwrap(), b"the repaired original");
    }
    fs::rename(replaced, root.path().join("held-foreign")).unwrap();
    fs::rename(&held, replaced).unwrap();
    let resumed = engine
        .resume_transfer(&restored, &cancel, |_| TransferReply::Continue)
        .unwrap();
    assert!(resumed.failures.is_empty());
    assert_eq!(
        fs::read(output.join("z-retry")).unwrap(),
        b"the repaired original"
    );
    assert_eq!(
        fs::read(output.join("a-completed")).unwrap(),
        b"older completed bytes"
    );
}

#[test]
fn future_or_relative_transfer_checkpoints_are_refused_before_execution() {
    let empty = TransferContinuation::retry_failed(
        TransferKind::Copy,
        std::path::PathBuf::from("/private-destination"),
        &[],
    );
    let encoded = serde_json::to_value(&empty).unwrap();
    let mut future = encoded.clone();
    future["version"] = 255.into();
    assert!(serde_json::from_value::<TransferContinuation>(future).is_err());
    let mut relative = encoded;
    relative["destination"] = "relative-destination".into();
    assert!(serde_json::from_value::<TransferContinuation>(relative).is_err());
}

#[test]
fn recycled_directory_tuples_cannot_authorize_a_replacement_or_parent_after_decode() {
    for replaced in [
        "source",
        "source-parent",
        "destination",
        "destination-parent",
    ] {
        let root = tempfile::tempdir().unwrap();
        let source_parent = root.path().join("source-parent");
        let destination = root.path().join("destination");
        let source = source_parent.join("project");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(source.join("a-done"), b"Completed original").unwrap();
        let failed = source.join("b-failed");
        rustix::fs::mkfifoat(rustix::fs::CWD, &failed, rustix::fs::Mode::RUSR).unwrap();
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        let result = engine
            .transfer(
                &TransferRequest {
                    kind: TransferKind::Copy,
                    sources: vec![source.clone()],
                    destination: destination.clone(),
                },
                &Cancellation::default(),
                |_| TransferReply::Continue,
            )
            .unwrap();
        let mut saved = serde_json::to_value(result.retry_continuation.unwrap()).unwrap();
        let target = destination.join("project");
        let before = fs::metadata(target.join("a-done")).unwrap();
        drop(engine);
        fs::remove_file(&failed).unwrap();
        fs::write(&failed, b"Repaired original").unwrap();
        let parked = root.path().join("parked-original");
        match replaced {
            "source" => {
                fs::rename(&source, &parked).unwrap();
                fs::create_dir(&source).unwrap();
                fs::write(&failed, b"Replacement source bytes").unwrap();
            }
            "source-parent" => {
                fs::rename(&source_parent, &parked).unwrap();
                fs::create_dir(&source_parent).unwrap();
                fs::rename(parked.join("project"), &source).unwrap();
            }
            "destination" => {
                fs::rename(&target, &parked).unwrap();
                fs::create_dir(&target).unwrap();
            }
            _ => {
                fs::rename(&destination, &parked).unwrap();
                fs::create_dir(&destination).unwrap();
                fs::rename(parked.join("project"), &target).unwrap();
            }
        }
        fs::write(target.join("a-done"), b"Newer completed bytes").unwrap();
        // Emulate recycled inode tuples without relying on allocator timing.
        // Durable original tokens, when present, are deliberately unchanged.
        let finish = &mut saved["directories"][0]["finish"];
        let identity = |path: &std::path::Path| {
            let m = fs::symlink_metadata(path).unwrap();
            serde_json::json!({"device":m.dev(),"inode":m.ino()})
        };
        finish["identity"] = identity(&source);
        finish["parent_identity"] = identity(&source_parent);
        finish["destination_identity"] = identity(&target);
        let plan: TransferContinuation = serde_json::from_value(saved).unwrap();
        let engine = OperationEngine::open(root.path().join("history")).unwrap();
        let refused = engine
            .resume_transfer(&plan, &Cancellation::default(), |_| TransferReply::Continue)
            .unwrap();
        assert_eq!(
            refused.completed, 0,
            "{replaced}: replacement was authorized"
        );
        assert!(
            !refused.failures.is_empty() || refused.needs_attention,
            "{replaced}: missing refusal"
        );
        assert!(
            !target.join("b-failed").exists(),
            "{replaced}: replacement was copied"
        );
        assert_eq!(
            fs::read(target.join("a-done")).unwrap(),
            b"Newer completed bytes"
        );
        if replaced != "destination" {
            let after = fs::metadata(target.join("a-done")).unwrap();
            assert_eq!((before.dev(), before.ino()), (after.dev(), after.ino()));
        }
    }
}

#[test]
fn older_checkpoint_versions_cannot_smuggle_original_traversal_facts() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        source.join("failed"),
        rustix::fs::Mode::RUSR,
    )
    .unwrap();
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let outcome = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source],
                destination,
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    let encoded = serde_json::to_value(outcome.retry_continuation.unwrap()).unwrap();
    assert_eq!(encoded["version"], 3);
    assert!(encoded["directories"][0]["finish"]["original"].is_object());
    for version in [1, 2] {
        let mut older = encoded.clone();
        older["version"] = version.into();
        if version == 1 {
            older.as_object_mut().unwrap().remove("restart");
        }
        assert!(serde_json::from_value::<TransferContinuation>(older.clone()).is_err());
        for directory in older["directories"].as_array_mut().unwrap() {
            directory["finish"]
                .as_object_mut()
                .unwrap()
                .remove("original");
        }
        assert!(serde_json::from_value::<TransferContinuation>(older).is_ok());
    }
}
