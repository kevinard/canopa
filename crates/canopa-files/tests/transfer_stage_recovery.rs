use canopa_files::{
    Cancellation,
    operations::OperationEngine,
    transfers::{TransferEvent, TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    os::unix::{
        fs::{FileTypeExt, MetadataExt, symlink},
        process::ExitStatusExt,
    },
    path::{Path, PathBuf},
};

fn killed_copy(root: &Path, output: &Path, kind: TransferKind) -> PathBuf {
    fs::write(root.join("item"), vec![0xab; 192 * 1024]).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cases::transfer_stage_recovery::killed_copy_helper",
        ])
        .env("CANOPA_STAGE_RECOVERY_ROOT", root)
        .env("CANOPA_STAGE_RECOVERY_OUTPUT", output)
        .env(
            "CANOPA_STAGE_RECOVERY_KIND",
            if kind == TransferKind::Move {
                "Move"
            } else {
                "Copy"
            },
        )
        .status()
        .unwrap();
    assert_eq!(status.signal(), Some(9));
    serde_json::from_slice(&fs::read(root.join("stage.json")).unwrap()).unwrap()
}

#[test]
fn a_killed_copy_cleans_only_the_original_partial_payload() {
    for kind in [TransferKind::Copy, TransferKind::Move] {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::Builder::new()
            .prefix(".canopa-copy-cleanup-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        if kind == TransferKind::Move
            && fs::metadata(root.path()).unwrap().dev()
                == fs::metadata(output.path()).unwrap().dev()
        {
            continue;
        }
        let stage = killed_copy(root.path(), output.path(), kind);
        assert_eq!(fs::metadata(stage.join("folder")).unwrap().len(), 64 * 1024);
        for _ in 0..2 {
            let engine = OperationEngine::open(root.path().join("history")).unwrap();
            assert!(
                engine
                    .interrupted_operations(&Cancellation::default())
                    .unwrap()
                    .is_empty()
            );
            assert!(!stage.exists());
            assert!(!output.path().join("item").exists());
            assert_eq!(
                fs::read(root.path().join("item")).unwrap(),
                vec![0xab; 192 * 1024]
            );
        }
    }
}

#[test]
fn a_killed_copy_keeps_replaced_payloads_stages_and_parents_across_two_restarts() {
    for kind in [TransferKind::Copy, TransferKind::Move] {
        for replaced in ["file", "link", "fifo", "directory", "stage", "parent"] {
            let root = tempfile::tempdir().unwrap();
            let output = tempfile::Builder::new()
                .prefix(".canopa-copy-refusal-")
                .tempdir_in(std::env::current_dir().unwrap())
                .unwrap();
            if kind == TransferKind::Move
                && fs::metadata(root.path()).unwrap().dev()
                    == fs::metadata(output.path()).unwrap().dev()
            {
                continue;
            }
            let stage = killed_copy(root.path(), output.path(), kind);
            let original_stage = output.path().join("parked-stage");
            let protected = output.path().join("protected");
            fs::write(&protected, b"Protected referent").unwrap();
            let original_payload = match replaced {
                "parent" => {
                    // Keep TempDir's own path present so its destructor removes
                    // only this test's replacement. Parked data has a separate owner.
                    let parked = tempfile::Builder::new()
                        .prefix(".canopa-copy-parked-")
                        .tempdir_in(std::env::current_dir().unwrap())
                        .unwrap();
                    let saved = parked.path().join("original-parent");
                    fs::rename(output.path(), &saved).unwrap();
                    fs::create_dir(output.path()).unwrap();
                    fs::create_dir(&stage).unwrap();
                    fs::write(stage.join("folder"), b"Foreign staged bytes").unwrap();
                    // Keep this owner alive through the assertions below.
                    (
                        saved.join(stage.file_name().unwrap()).join("folder"),
                        Some(parked),
                    )
                }
                "stage" => {
                    fs::rename(&stage, &original_stage).unwrap();
                    fs::create_dir(&stage).unwrap();
                    fs::write(stage.join("folder"), b"Foreign staged bytes").unwrap();
                    (original_stage.join("folder"), None)
                }
                _ => {
                    let parked = stage.join("original-payload");
                    fs::rename(stage.join("folder"), &parked).unwrap();
                    match replaced {
                        "link" => symlink(&protected, stage.join("folder")).unwrap(),
                        "fifo" => rustix::fs::mkfifoat(
                            rustix::fs::CWD,
                            stage.join("folder"),
                            rustix::fs::Mode::RUSR,
                        )
                        .unwrap(),
                        "directory" => {
                            fs::create_dir(stage.join("folder")).unwrap();
                            fs::write(stage.join("folder/sentinel"), b"Foreign staged bytes")
                                .unwrap();
                        }
                        _ => fs::write(stage.join("folder"), b"Foreign staged bytes").unwrap(),
                    }
                    (parked, None)
                }
            };
            let before = fs::metadata(&original_payload.0).unwrap();
            for _ in 0..2 {
                let engine = OperationEngine::open(root.path().join("history")).unwrap();
                assert!(
                    !engine
                        .interrupted_operations(&Cancellation::default())
                        .unwrap()
                        .is_empty(),
                    "{kind:?}/{replaced}"
                );
                assert!(
                    fs::symlink_metadata(stage.join("folder")).is_ok(),
                    "{kind:?}/{replaced}"
                );
                assert_eq!(
                    fs::metadata(&original_payload.0).unwrap().ino(),
                    before.ino()
                );
                assert_eq!(
                    fs::read(&original_payload.0).unwrap(),
                    vec![0xab; 64 * 1024]
                );
                assert_eq!(
                    fs::read(root.path().join("item")).unwrap(),
                    vec![0xab; 192 * 1024]
                );
                assert!(!output.path().join("item").exists());
                match replaced {
                    "fifo" => assert!(
                        fs::symlink_metadata(stage.join("folder"))
                            .unwrap()
                            .file_type()
                            .is_fifo()
                    ),
                    "link" => {
                        assert_eq!(fs::read_link(stage.join("folder")).unwrap(), protected);
                        assert_eq!(fs::read(&protected).unwrap(), b"Protected referent");
                    }
                    "directory" => assert_eq!(
                        fs::read(stage.join("folder/sentinel")).unwrap(),
                        b"Foreign staged bytes"
                    ),
                    _ => assert_eq!(
                        fs::read(stage.join("folder")).unwrap(),
                        b"Foreign staged bytes"
                    ),
                }
            }
        }
    }
}

#[test]
#[ignore = "invoked by killed-copy parents in an isolated process"]
fn killed_copy_helper() {
    let Some(root) = std::env::var_os("CANOPA_STAGE_RECOVERY_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let output = PathBuf::from(std::env::var_os("CANOPA_STAGE_RECOVERY_OUTPUT").unwrap());
    let kind = if std::env::var("CANOPA_STAGE_RECOVERY_KIND").unwrap() == "Move" {
        TransferKind::Move
    } else {
        TransferKind::Copy
    };
    let engine = OperationEngine::open(root.join("history")).unwrap();
    engine
        .transfer(
            &TransferRequest {
                kind,
                sources: vec![root.join("item")],
                destination: output.clone(),
            },
            &Cancellation::default(),
            |event| {
                if matches!(event, TransferEvent::Progress { bytes, .. } if bytes > 0) {
                    let stage = fs::read_dir(&output)
                        .unwrap()
                        .map(Result::unwrap)
                        .map(|entry| entry.path())
                        .find(|path| {
                            path.file_name()
                                .unwrap()
                                .as_encoded_bytes()
                                .starts_with(b".canopa-files-stage-")
                        })
                        .unwrap();
                    fs::write(root.join("stage.json"), serde_json::to_vec(&stage).unwrap())
                        .unwrap();
                    rustix::process::kill_process(
                        rustix::process::getpid(),
                        rustix::process::Signal::KILL,
                    )
                    .unwrap();
                    unreachable!();
                }
                TransferReply::Continue
            },
        )
        .unwrap();
    panic!("helper did not reach a staged data write");
}
