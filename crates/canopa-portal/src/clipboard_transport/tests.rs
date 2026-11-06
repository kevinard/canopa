use super::*;
use std::{
    fs::File,
    io::{Read, Write},
    time::Instant,
};

#[test]
fn replacement_workers_share_serials_without_reusing_exhausted_values() {
    let serials = Serials(Arc::new(AtomicU32::new(0)));
    assert_eq!(serials.next(), Some(1));
    assert_eq!(serials.clone().next(), Some(2));
    serials.0.store(u32::MAX - 1, Ordering::Release);
    assert_eq!(serials.clone().next(), Some(u32::MAX));
    assert_eq!(serials.next(), None);
    assert_eq!(serials.clone().next(), None);
}

#[test]
fn dropping_a_pump_revokes_duplicated_application_descriptors() {
    let (source, app_writer) = pipes::pipe().unwrap();
    let (app_reader, target) = pipes::pipe().unwrap();
    let pump = pipes::Pump::new(source, target).unwrap();
    let mut writer = File::from(app_writer).try_clone().unwrap();
    let mut reader = File::from(app_reader).try_clone().unwrap();
    drop(pump);
    assert_eq!(
        writer.write(b"after revocation").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(reader.read(&mut [0; 1]).unwrap(), 0);
}

#[test]
fn bounded_pump_preserves_binary_bytes_and_eof_under_backpressure() {
    let (source, app_writer) = pipes::pipe().unwrap();
    let (app_reader, target) = pipes::pipe().unwrap();
    let mut pump = pipes::Pump::new(source, target).unwrap();
    let expected: Vec<_> = (0..2 * 1024 * 1024)
        .map(|index| (index % 251) as u8)
        .collect();
    let bytes = expected.clone();
    let producer = std::thread::spawn(move || File::from(app_writer).write_all(&bytes).unwrap());
    let mut reader = File::from(app_reader);
    rustix::fs::fcntl_setfl(&reader, rustix::fs::OFlags::NONBLOCK).unwrap();
    let mut actual = Vec::new();
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        let finished = pump.step().unwrap();
        let mut chunk = [0; 503];
        match reader.read(&mut chunk) {
            Ok(count) => actual.extend_from_slice(&chunk[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("{error}"),
        }
        if finished {
            break;
        }
        assert!(Instant::now() < end);
        std::thread::yield_now();
    }
    drop(pump);
    reader.read_to_end(&mut actual).unwrap();
    producer.join().unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn malformed_or_reserved_metadata_is_rejected_without_normalizing_valid_types() {
    for types in [
        vec!["".into()],
        vec!["text/plain\n".into()],
        vec![format!("{OWNER_PREFIX}forged")],
        vec!["x".repeat(MAX_MIME_BYTES + 1)],
        vec!["text/plain".into(); MAX_TYPES + 1],
        vec!["text/plain".into(), "text/plain".into()],
    ] {
        assert!(validate_types(&types).is_err());
    }
    assert!(validate_types(&[]).is_ok());
    assert!(
        validate_types(&[
            "text/plain;charset=utf-8".into(),
            "application/octet-stream".into()
        ])
        .is_ok()
    );
}

/// Launched only by scripts/test-clipboard-transport.py on its disposable display.
#[tokio::test]
#[ignore = "requires the private clipboard transport compositor fixture"]
async fn native_clipboard_transport() {
    use std::{path::PathBuf, process::Stdio};
    use tokio::{
        io::AsyncWriteExt,
        process::{Child, Command},
    };
    let runtime = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
    assert!(runtime.starts_with(std::env::temp_dir()));
    assert!(
        runtime
            .to_string_lossy()
            .contains("canopa-clipboard-transport-")
    );
    assert!(std::env::var_os("WAYLAND_SOCKET").is_none());
    let pid: u32 = std::env::var("CANOPA_CLIPBOARD_NIRI_PID")
        .unwrap()
        .parse()
        .unwrap();
    let output = PathBuf::from(std::env::var_os("CANOPA_CLIPBOARD_OUTPUT").unwrap());
    assert!(output.is_dir());
    let mut checks = Vec::new();
    async fn snapshot(
        changes: &mut watch::Receiver<Snapshot>,
        predicate: impl Fn(&Snapshot) -> bool,
    ) -> Snapshot {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let snapshot = changes.borrow_and_update().clone();
                assert!(snapshot.error.is_none(), "{snapshot:?}");
                if predicate(&snapshot) {
                    return snapshot;
                }
                changes.changed().await.unwrap();
            }
        })
        .await
        .expect("clipboard snapshot")
    }
    async fn source(mime: &str, bytes: &[u8], primary: bool) -> Child {
        let mut command = Command::new("/usr/bin/wl-copy");
        command.args(["--foreground", "--type", mime]);
        if primary {
            command.arg("--primary");
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        tokio::time::timeout(Duration::from_secs(5), stdin.write_all(bytes))
            .await
            .unwrap()
            .unwrap();
        drop(stdin);
        child
    }
    fn paste(mime: &str, primary: bool) -> Child {
        let mut command = Command::new("/usr/bin/wl-paste");
        command.args(["--no-newline", "--type", mime]);
        if primary {
            command.arg("--primary");
        }
        command
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap()
    }
    async fn pasted(child: Child) -> std::process::Output {
        tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
            .await
            .expect("clipboard peer EOF")
            .unwrap()
    }
    fn read(fd: OwnedFd) -> Vec<u8> {
        let mut file = File::from(fd);
        rustix::fs::fcntl_setfl(&file, rustix::fs::OFlags::NONBLOCK).unwrap();
        let mut bytes = Vec::new();
        let mut chunk = [0; 4093];
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            match file.read(&mut chunk) {
                Ok(0) => return bytes,
                Ok(count) => bytes.extend_from_slice(&chunk[..count]),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(error) => panic!("{error}"),
            }
            assert!(Instant::now() < end, "clipboard read did not finish");
        }
    }
    async fn transfer(events: &mut asynchronous::Receiver<Transfer>) -> Transfer {
        tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("clipboard transfer request")
            .expect("clipboard events")
    }
    assert!(Bridge::start(std::process::id()).await.is_err());
    let mut bridge = Bridge::start(pid).await.unwrap();
    let mut changes = bridge.snapshots();
    let mut events = bridge.take_transfers().unwrap();
    checks.push("named-display-pinned-to-the-verified-compositor-pid");
    const TEXT: &str = "text/plain;charset=utf-8";
    const BINARY: &str = "application/octet-stream";
    let mut primary = source(TEXT, b"independent primary", true).await;
    let primary_end = Instant::now() + Duration::from_secs(5);
    loop {
        if pasted(paste(TEXT, true)).await.stdout == b"independent primary" {
            break;
        }
        assert!(Instant::now() < primary_end);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let mut local = source(TEXT, b"local clipboard", false).await;
    let initial = snapshot(&mut changes, |s| {
        s.mime_types.contains(&TEXT.into()) && !s.session_is_owner
    })
    .await;
    assert_eq!(
        read(bridge.read(initial.generation, TEXT.into()).await.unwrap()),
        b"local clipboard"
    );
    checks.push("external-wl-copy-selection-read-through-owned-readable-pipe");
    let payload: Vec<u8> = (0..2 * 1024 * 1024)
        .map(|index| (index % 251) as u8)
        .collect();
    let mut binary = source(BINARY, &payload, false).await;
    let changed = snapshot(&mut changes, |s| {
        s.generation != initial.generation && s.mime_types.contains(&BINARY.into())
    })
    .await;
    assert!(bridge.read(initial.generation, TEXT.into()).await.is_err());
    assert_eq!(
        read(
            bridge
                .read(changed.generation, BINARY.into())
                .await
                .unwrap()
        ),
        payload
    );
    assert!(
        bridge
            .read(changed.generation, "application/unknown".into())
            .await
            .is_err()
    );
    checks.push("binary-two-megabyte-stream-is-exact-and-stale-generation-read-is-rejected");
    bridge
        .publish(vec![TEXT.into(), BINARY.into()])
        .await
        .unwrap();
    let owned = snapshot(&mut changes, |s| s.session_is_owner).await;
    assert_eq!(owned.mime_types, vec![TEXT.to_string(), BINARY.to_string()]);
    assert!(
        owned
            .mime_types
            .iter()
            .all(|mime| !mime.starts_with(OWNER_PREFIX))
    );
    assert_eq!(
        pasted(paste(TEXT, true)).await.stdout,
        b"independent primary"
    );
    checks.push(
        "publication-correlates-ownership-filters-session-marker-and-preserves-primary-selection",
    );
    let receiving = paste(BINARY, false);
    let request = transfer(&mut events).await;
    assert_eq!(request.mime_type, BINARY);
    assert!(bridge.write_done(request.serial, true).await.is_err());
    let mut writer = File::from(bridge.write(request.serial).await.unwrap());
    assert!(bridge.write(request.serial).await.is_err());
    // Drain the peer output while producing; otherwise its stdout pipe can
    // backpressure the entire chain before write_all returns.
    let receiving = tokio::spawn(pasted(receiving));
    let sending = payload.clone();
    let writer = tokio::task::spawn_blocking(move || {
        writer.write_all(&sending).unwrap();
        writer
    })
    .await
    .unwrap();
    bridge.write_done(request.serial, true).await.unwrap();
    assert!(bridge.write_done(request.serial, true).await.is_err());
    assert!(
        !receiving.is_finished(),
        "peer saw EOF before writer closed"
    );
    drop(writer);
    let received = receiving.await.unwrap();
    assert!(received.status.success());
    assert_eq!(received.stdout, payload);
    assert!(bridge.write(request.serial).await.is_err());
    checks.push("wl-paste-reads-exact-bytes-and-write-completion-requires-claimed-serial-and-eof");
    let receiving = paste(TEXT, false);
    let request = transfer(&mut events).await;
    let mut writer = File::from(bridge.write(request.serial).await.unwrap())
        .try_clone()
        .unwrap();
    bridge.write_done(request.serial, false).await.unwrap();
    assert_eq!(
        writer.write(b"late").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert!(pasted(receiving).await.stdout.is_empty());
    let receiving = paste(TEXT, false);
    let declined = transfer(&mut events).await;
    bridge.write_done(declined.serial, false).await.unwrap();
    assert!(pasted(receiving).await.stdout.is_empty());
    assert!(bridge.write(declined.serial).await.is_err());
    checks.push("declining-before-or-after-write-retires-the-serial-and-revokes-duplicated-writer");
    let receiving = paste(TEXT, false);
    let request = transfer(&mut events).await;
    let mut writer = File::from(bridge.write(request.serial).await.unwrap());
    let mut replacement = source(BINARY, b"new local bytes", false).await;
    let replaced = snapshot(&mut changes, |s| {
        !s.session_is_owner
            && s.generation != owned.generation
            && s.mime_types.contains(&BINARY.into())
    })
    .await;
    assert_eq!(
        writer.write(b"stale source").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert!(bridge.write(request.serial).await.is_err());
    assert!(pasted(receiving).await.stdout.is_empty());
    assert_eq!(
        read(
            bridge
                .read(replaced.generation, BINARY.into())
                .await
                .unwrap()
        ),
        b"new local bytes"
    );
    bridge.stop().await.unwrap();
    assert_eq!(
        pasted(paste(BINARY, false)).await.stdout,
        b"new local bytes"
    );
    assert!(!changes.borrow().ready);
    checks.push(
        "source-replacement-cancels-transfers-and-closing-preserves-another-owners-selection",
    );
    let mut closing = Bridge::start(pid).await.unwrap();
    let mut closing_events = closing.take_transfers().unwrap();
    closing.publish(vec![BINARY.into()]).await.unwrap();
    let generation = closing.snapshots().borrow().generation;
    let read_fd = closing.read(generation, BINARY.into()).await.unwrap();
    let pending = transfer(&mut closing_events).await;
    assert_eq!(pending.mime_type, BINARY);
    let mut still_held_reader = File::from(read_fd).try_clone().unwrap();
    let mut still_held_writer = File::from(closing.write(pending.serial).await.unwrap())
        .try_clone()
        .unwrap();
    let mut stopped = closing.snapshots();
    closing.revoker().unwrap().revoke();
    tokio::time::timeout(Duration::from_secs(2), async {
        while stopped.borrow().ready {
            stopped.changed().await.unwrap();
        }
    })
    .await
    .expect("revoked clipboard worker stopped");
    assert_eq!(
        still_held_writer.write(b"revoked").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(still_held_reader.read(&mut [0; 1]).unwrap(), 0);
    closing.stop().await.unwrap();
    let retired = Bridge::start(pid).await.unwrap();
    assert!(
        retired.snapshots().borrow().mime_types.is_empty(),
        "closed source still advertised"
    );
    retired.stop().await.unwrap();
    checks.push("grant-revoker-cancels-both-app-held-pipes-before-bridge-owner-is-dropped");
    assert_eq!(
        pasted(paste(TEXT, true)).await.stdout,
        b"independent primary"
    );
    for child in [&mut primary, &mut local, &mut binary, &mut replacement] {
        if child.try_wait().unwrap().is_none() {
            child.kill().await.unwrap();
        }
        let _ = child.wait().await;
    }
    std::fs::write(output.join("native-result.json"), serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","checks":checks,"binary_payload_bytes":payload.len(),"compositor_pid":pid})).unwrap()).unwrap();
}

#[test]
fn regular_files_cannot_block_the_stream_worker() {
    let (reader, app_writer) = pipes::pipe().unwrap();
    let target = tempfile::tempfile().unwrap();
    assert!(pipes::Pump::new(reader, target.into()).is_err());
    assert_eq!(
        File::from(app_writer).write(b"retired").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
}
