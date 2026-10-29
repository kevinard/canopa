use canopa_files::{
    Cancellation,
    operations::OperationEngine,
    transfers::{TransferKind, TransferReply, TransferRequest},
};
use std::{
    fs,
    os::unix::{
        ffi::OsStringExt,
        fs::{MetadataExt, PermissionsExt, symlink},
    },
    time::{Duration, SystemTime},
};

#[test]
fn shared_copy_retains_the_documented_metadata_and_link_policy() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let output = root.path().join("output");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&output).unwrap();
    let raw = std::ffi::OsString::from_vec(b"file-\xff".to_vec());
    let file = fs::File::create(source.join(&raw)).unwrap();
    file.set_len(1024 * 1024).unwrap();
    file.set_permissions(fs::Permissions::from_mode(0o640))
        .unwrap();
    let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(123456789);
    let accessed = SystemTime::UNIX_EPOCH + Duration::from_secs(123456000);
    file.set_times(
        fs::FileTimes::new()
            .set_modified(modified)
            .set_accessed(accessed),
    )
    .unwrap();
    let xattrs = rustix::fs::fsetxattr(
        &file,
        c"user.canopa-proof",
        b"private metadata",
        rustix::fs::XattrFlags::empty(),
    );
    assert!(xattrs.is_ok() || matches!(xattrs, Err(rustix::io::Errno::NOTSUP)));
    fs::write(source.join("linked"), b"hard-link group").unwrap();
    fs::hard_link(source.join("linked"), source.join("alias")).unwrap();
    fs::write(source.join("empty"), b"").unwrap();
    let target = std::ffi::OsString::from_vec(b"missing-\xfe".to_vec());
    symlink(&target, source.join("dangling")).unwrap();
    let engine = OperationEngine::open(root.path().join("history")).unwrap();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: output.clone(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert!(result.failures.is_empty());
    let copied = output.join("source");
    let metadata = fs::metadata(copied.join(&raw)).unwrap();
    assert_eq!(metadata.mode() & 0o7777, 0o640);
    assert_eq!(metadata.modified().unwrap(), modified);
    assert_eq!(metadata.accessed().unwrap(), accessed);
    assert_eq!(fs::read(copied.join(&raw)).unwrap(), vec![0; 1024 * 1024]);
    // These limitations are deliberate baseline behavior, not cp -a claims.
    assert!(metadata.blocks() > file.metadata().unwrap().blocks());
    assert_ne!(
        fs::metadata(copied.join("linked")).unwrap().ino(),
        fs::metadata(copied.join("alias")).unwrap().ino()
    );
    if xattrs.is_ok() {
        assert_eq!(
            rustix::fs::fgetxattr(
                fs::File::open(copied.join(&raw)).unwrap(),
                c"user.canopa-proof",
                &mut [0u8; 64]
            )
            .unwrap_err(),
            rustix::io::Errno::NODATA
        );
    }
    assert!(fs::read(copied.join("empty")).unwrap().is_empty());
    assert_eq!(
        fs::read_link(copied.join("dangling")).unwrap().as_os_str(),
        target
    );
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!copied.exists());
    assert!(source.join(raw).exists());
}

#[test]
fn independent_copy_jobs_do_not_share_buffered_content() {
    std::thread::scope(|threads| {
        for byte in [0x61, 0xb7] {
            threads.spawn(move || {
                let root = tempfile::tempdir().unwrap();
                let source = root.path().join("source");
                let output = root.path().join("output");
                fs::create_dir(&output).unwrap();
                let bytes = vec![byte; 1024 * 1024];
                fs::write(&source, &bytes).unwrap();
                let engine = OperationEngine::open(root.path().join("history")).unwrap();
                let result = engine
                    .transfer(
                        &TransferRequest {
                            kind: TransferKind::Copy,
                            sources: vec![source],
                            destination: output.clone(),
                        },
                        &Cancellation::default(),
                        |_| TransferReply::Continue,
                    )
                    .unwrap();
                assert_eq!(result.completed, 1);
                assert!(result.failures.is_empty());
                assert_eq!(fs::read(output.join("source")).unwrap(), bytes);
                engine.undo(&Cancellation::default()).unwrap();
            });
        }
    });
}

struct MappedWriter(std::process::Child);
impl Drop for MappedWriter {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn receipt_case(mapped_source: bool, kind: TransferKind) {
    use std::io::{BufRead, Write};
    let root = tempfile::Builder::new()
        .prefix("canopa-copy-receipt-")
        .tempdir()
        .unwrap()
        .keep();
    eprintln!(
        "private receipt fixture (retained on failure): {}",
        root.display()
    );
    let source = root.join("source");
    let output = if kind == TransferKind::Move {
        let device = fs::metadata(&root).unwrap().dev();
        [std::env::current_dir().unwrap(), "/dev/shm".into()]
            .into_iter()
            .find_map(|parent| {
                (fs::metadata(&parent).ok()?.dev() != device)
                    .then(|| {
                        tempfile::Builder::new()
                            .prefix(".canopa-copy-receipt-")
                            .tempdir_in(parent)
                            .ok()
                    })
                    .flatten()
            })
            .expect("two writable test filesystems")
            .keep()
    } else {
        root.join("output")
    };
    eprintln!("private output fixture: {}", output.display());
    fs::write(&source, b"AAAAAAAA").unwrap();
    fs::create_dir_all(&output).unwrap();
    let mut writer = mapped_source.then(|| {
        let child = std::process::Command::new("python3").args(["-c", r#"
import mmap, os, sys
with open(sys.argv[1], 'r+b', buffering=0) as source:
    with mmap.mmap(source.fileno(), 0) as data:
        data[0] = ord('A')
        print('ready', flush=True)
        assert sys.stdin.readline().strip() == 'change'
        before = os.fstat(source.fileno())
        data[0] = ord('B')
        after = os.fstat(source.fileno())
        assert (before.st_size, before.st_mode, before.st_mtime_ns, before.st_ctime_ns) == (after.st_size, after.st_mode, after.st_mtime_ns, after.st_ctime_ns)
        print('changed', flush=True)
        sys.stdin.readline()
"#]).arg(&source).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
        MappedWriter(child)
    });
    let mut notices = writer
        .as_mut()
        .map(|w| std::io::BufReader::new(w.0.stdout.take().unwrap()));
    if let Some(notices) = &mut notices {
        let mut line = String::new();
        notices.read_line(&mut line).unwrap();
        assert_eq!(line.trim(), "ready");
    }
    let engine = OperationEngine::open(root.join("history")).unwrap();
    let mut edited = false;
    let result = engine.transfer(&TransferRequest {
        kind, sources: vec![source.clone()], destination: output.clone(),
    }, &Cancellation::default(), |event| {
        if !edited && matches!(event, canopa_files::transfers::TransferEvent::Progress { bytes, .. } if bytes > 0) {
            edited = true;
            if let Some(writer) = &mut writer {
                writer.0.stdin.as_mut().unwrap().write_all(b"change\n").unwrap();
                let mut line = String::new(); notices.as_mut().unwrap().read_line(&mut line).unwrap(); assert_eq!(line.trim(), "changed");
            } else {
                let stage = fs::read_dir(&output).unwrap().map(|e| e.unwrap().path()).find(|p| p.file_name().unwrap().as_encoded_bytes().starts_with(b".canopa-files-stage-")).unwrap();
                fs::write(stage.join("folder"), b"BAAAAAAA").unwrap();
            }
        }
        if matches!(event, canopa_files::transfers::TransferEvent::Conflict { .. }) {
            TransferReply::Resolve {
                choice: canopa_files::transfers::ConflictChoice::Replace,
                apply_to_remaining: false,
            }
        } else {
            TransferReply::Continue
        }
    });
    drop(writer);
    assert!(edited);
    let result = result.unwrap();
    if kind == TransferKind::Copy {
        assert_eq!(result.completed, 1);
        assert!(result.failures.is_empty());
    } else {
        assert!(!result.failures.is_empty());
    }
    let source_bytes = if mapped_source {
        b"BAAAAAAA"
    } else {
        b"AAAAAAAA"
    };
    let copied_bytes = if mapped_source {
        b"AAAAAAAA"
    } else {
        b"BAAAAAAA"
    };
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    assert_eq!(fs::read(output.join("source")).unwrap(), copied_bytes);
    // Reopen the persisted history: Undo must recognize the actual output,
    // including when it differs from the current source, and leave that source.
    drop(engine);
    let engine = OperationEngine::open(root.join("history")).unwrap();
    engine.undo(&Cancellation::default()).unwrap();
    assert!(!output.join("source").exists());
    assert_eq!(fs::read(&source).unwrap(), source_bytes);
    drop(engine);
    if kind == TransferKind::Move {
        fs::remove_dir_all(output).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn copy_trusts_upstream_and_undo_fingerprints_the_output_after_a_mapped_source_edit() {
    receipt_case(true, TransferKind::Copy);
}

#[test]
fn undo_records_actual_staged_bytes_instead_of_hashing_the_source() {
    receipt_case(false, TransferKind::Copy);
}

#[test]
fn cross_filesystem_move_retains_a_mapped_source_that_differs_from_the_copy() {
    receipt_case(true, TransferKind::Move);
}

#[test]
fn cross_filesystem_move_retains_its_source_when_staged_bytes_differ() {
    receipt_case(false, TransferKind::Move);
}

#[test]
fn an_old_sha_receipt_is_refused_without_removing_the_copy_or_source() {
    use sha2::{Digest, Sha256};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let output = root.path().join("output");
    let history = root.path().join("history");
    let bytes = b"development copy";
    fs::write(&source, bytes).unwrap();
    fs::create_dir(&output).unwrap();
    let engine = OperationEngine::open(&history).unwrap();
    let result = engine
        .transfer(
            &TransferRequest {
                kind: TransferKind::Copy,
                sources: vec![source.clone()],
                destination: output.clone(),
            },
            &Cancellation::default(),
            |_| TransferReply::Continue,
        )
        .unwrap();
    assert_eq!(result.completed, 1);
    drop(engine);
    let legacy = {
        let db = rusqlite::Connection::open(&history).unwrap();
        let body: String = db
            .query_row("SELECT body FROM changes", [], |row| row.get(0))
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&body).unwrap();
        let fingerprint = value["fingerprint"].as_object_mut().unwrap();
        assert!(fingerprint.remove("xxh3_128").is_some());
        fingerprint.insert(
            "digest".into(),
            serde_json::json!(Sha256::digest(bytes).to_vec()),
        );
        let legacy = value.to_string();
        db.execute("UPDATE changes SET body=?1", [&legacy]).unwrap();
        legacy
    };
    let error = match OperationEngine::open(&history) {
        Err(error) => error,
        Ok(engine) => engine.undo(&Cancellation::default()).unwrap_err(),
    };
    assert!(
        error.to_string().contains("unknown field `digest`"),
        "old receipt must be rejected by format: {error}"
    );
    assert_eq!(fs::read(&source).unwrap(), bytes);
    assert_eq!(fs::read(output.join("source")).unwrap(), bytes);
    let db = rusqlite::Connection::open(history).unwrap();
    assert_eq!(
        db.query_row("SELECT body FROM changes", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        legacy
    );
}
