use super::*;
use crate::print::file_target::Reviewed;
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::Path,
    time::Duration,
};

fn pin(path: &Path) -> Pinned {
    let review = Reviewed::open(path).unwrap();
    let (send, receive) = UnixStream::pair().unwrap();
    review.send(&send).unwrap();
    drop(send);
    Pinned::receive(&receive, review.selection().clone()).unwrap()
}

#[tokio::test]
#[ignore = "requires CANOPA_PRINT_FILE_BINARY pointing to the normal workspace executable"]
async fn native_file_client_fixture() {
    let binary = std::env::var_os("CANOPA_PRINT_FILE_BINARY").unwrap();
    let output = std::env::var_os("CANOPA_PRINT_FILE_OUTPUT").unwrap();
    let folder = tempfile::tempdir().unwrap();
    let client = Client::new(DialogProcess {
        executable: binary.clone().into(),
        arguments: Vec::new(),
    });
    let (_cancel, cancelled) = watch::channel(false);
    let mut source = tempfile::tempfile().unwrap();
    source.write_all(b"skip%PDF-1.7\nfrozen bytes").unwrap();
    source.seek(SeekFrom::Start(4)).unwrap();
    let frozen = client
        .snapshot(source.try_clone().unwrap().into(), cancelled.clone())
        .await
        .unwrap()
        .unwrap();
    source.write_all(b"changed source").unwrap();
    let mut checks = Vec::new();
    for existing in [false, true] {
        let target = folder
            .path()
            .join(if existing { "existing.pdf" } else { "new.pdf" });
        let original = folder.path().join("retained.pdf");
        if existing {
            fs::write(&target, b"reviewed original").unwrap();
            fs::hard_link(&target, &original).unwrap();
        }
        let result = client
            .publish_file(&frozen, pin(&target), Format::Pdf, cancelled.clone())
            .await
            .unwrap()
            .unwrap();
        assert!(result.published && !result.uncertain && result.recovery.is_none());
        assert_eq!(fs::read(&target).unwrap(), b"%PDF-1.7\nfrozen bytes");
        if existing {
            assert_eq!(fs::read(original).unwrap(), b"reviewed original");
        }
        checks.push(if existing {
            "reviewed replacement keeps old hardlink"
        } else {
            "new output uses frozen bytes"
        });
    }
    let target = folder.path().join("format.pdf");
    assert!(
        client
            .publish_file(&frozen, pin(&target), Format::Ps, cancelled.clone())
            .await
            .is_err()
    );
    assert!(!target.exists());
    checks.push("mismatched format leaves no output");
    let target = folder.path().join("late.pdf");
    let reviewed = pin(&target);
    fs::write(&target, b"late file").unwrap();
    assert!(
        client
            .publish_file(&frozen, reviewed, Format::Pdf, cancelled.clone())
            .await
            .is_err()
    );
    assert_eq!(fs::read(&target).unwrap(), b"late file");
    checks.push("late file is preserved");
    let (cancel, stopped) = watch::channel(true);
    let target = folder.path().join("cancelled.pdf");
    assert!(
        client
            .publish_file(&frozen, pin(&target), Format::Pdf, stopped)
            .await
            .unwrap()
            .is_none()
    );
    assert!(!target.exists());
    drop(cancel);
    checks.push("pre-cancelled output never starts");

    // A controllable worker boundary tests Client's independently owned cleanup.
    // The settlement operation execs the actual normal production worker.
    let marker = folder.path().join("claimed");
    let wrapper = folder.path().join("worker.py");
    fs::write(
        &wrapper,
        format!(
            r#"#!/usr/bin/python3
import json, os, select, sys
raw = sys.stdin.buffer.readline()
request = json.loads(raw)
op = request['operation']
if op['operation'] != 'publish_file':
    fd = os.memfd_create('worker-input', os.MFD_CLOEXEC)
    os.write(fd, raw)
    os.lseek(fd, 0, os.SEEK_SET)
    os.dup2(fd, 0)
    os.close(fd)
    os.execv({binary:?}, [{binary:?}, '--print-worker'])
assert op['operation'] == 'publish_file'
fds = op['descriptors']
record = os.open('/proc/self/fd/' + str(fds[-1]), os.O_RDWR | os.O_CLOEXEC)
os.pwrite(record, b'{{"phase":"claiming","claimed":null}}\n', os.fstat(record).st_size)
os.fsync(record)
name = os.path.basename(bytes(op['publication']['selection']['path']))
os.rename(name, 'previous', src_dir_fd=fds[0], dst_dir_fd=fds[-3])
with open({marker:?}, 'w') as output: output.write('claimed')
select.select([request['cancel']], [], [], 20)
sys.exit(2)
"#,
            binary = binary.to_str().unwrap(),
            marker = marker.to_str().unwrap()
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    let controlled = Client::new(DialogProcess {
        executable: wrapper,
        arguments: Vec::new(),
    });
    let target = folder.path().join("abandoned.pdf");
    fs::write(&target, b"recover this original").unwrap();
    let reviewed = pin(&target);
    let operation = tokio::spawn({
        let controlled = controlled.clone();
        let frozen = frozen.clone();
        async move {
            controlled
                .publish_file(&frozen, reviewed, Format::Pdf, cancelled)
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(10), async {
        while !marker.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(!target.exists());
    operation.abort();
    assert!(operation.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(15), controlled.idle())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"recover this original");
    assert!(!fs::read_dir(folder.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .as_encoded_bytes()
            .starts_with(b".canopa-print-")
    }));
    checks.push("dropped initiating future retires worker and restores original through production settlement");
    controlled.shutdown().await;
    client.shutdown().await;
    fs::write(
        output,
        serde_json::to_vec_pretty(&serde_json::json!({"status":"passed", "checks":checks}))
            .unwrap(),
    )
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires normal executable; exercises real FIFO-stalled file helpers and deadlines"]
async fn native_file_stalls() {
    let binary = std::env::var_os("CANOPA_PRINT_FILE_BINARY").unwrap();
    let output = std::env::var_os("CANOPA_PRINT_FILE_OUTPUT").unwrap();
    let mode = std::env::var("CANOPA_PRINT_FILE_MODE").unwrap();
    assert!(
        [
            "setup-cancel",
            "setup-drop",
            "verify-cancel",
            "cleanup-accepted",
            "cleanup-lost-receipt"
        ]
        .contains(&mode.as_str())
    );
    let folder = tempfile::tempdir().unwrap();
    let marker = folder.path().join("blocked.json");
    let fifo = folder.path().join("blocked.fifo");
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let blocked = if mode.starts_with("setup") {
        "prepare_file"
    } else if mode.starts_with("verify") {
        "verify_file"
    } else {
        "settle_file"
    };
    let wrapper = folder.path().join("worker.py");
    fs::write(&wrapper, format!(r#"#!/usr/bin/python3
import json,os,subprocess,sys
raw=sys.stdin.buffer.readline();request=json.loads(raw);op=request['operation']
if op['operation']=={blocked:?}:
    with open({marker:?},'w') as out: json.dump(dict(pid=os.getpid(),operation=op['operation']),out)
    os.open({fifo:?},os.O_RDONLY) # Real filesystem open blocks without a writer.
    sys.exit(3)
if {mode:?}=='cleanup-lost-receipt' and op['operation']=='publish_file':
    fds=list(set([request['cancel'],op['document'],*op['descriptors']]))
    child=subprocess.run([{binary:?},'--print-worker'],input=raw,stdout=subprocess.PIPE,pass_fds=fds)
    assert child.returncode==0 and json.loads(child.stdout)['event']=='published_file'
    sys.exit(2) # Actual commit, deliberately lost positive receipt.
fd=os.memfd_create('worker-input',os.MFD_CLOEXEC);os.write(fd,raw);os.lseek(fd,0,os.SEEK_SET);os.dup2(fd,0);os.close(fd)
os.execv({binary:?},[{binary:?},'--print-worker'])
"#, blocked=blocked, marker=marker.to_str().unwrap(), fifo=fifo.to_str().unwrap(),
        mode=mode, binary=binary.to_str().unwrap())).unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    let client = Client::new(DialogProcess {
        executable: wrapper,
        arguments: Vec::new(),
    });
    let target = folder.path().join("output.pdf");
    fs::write(&target, b"original").unwrap();
    let reviewed = pin(&target);
    let (cancel, cancelled) = watch::channel(false);
    let operation = tokio::spawn({
        let client = client.clone();
        let verify = mode.starts_with("verify");
        async move {
            if verify {
                client
                    .verify_file(&reviewed, cancelled)
                    .await
                    .map(|value| value.map(|()| None))
            } else {
                let frozen = fixture_document(b"%PDF-1.7\nworker fallback fixture");
                client
                    .publish_file(&frozen, reviewed, Format::Pdf, cancelled)
                    .await
                    .map(|value| value.map(Some))
            }
        }
    });
    tokio::time::timeout(Duration::from_secs(10), async {
        while !marker.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let blocked: serde_json::Value = serde_json::from_slice(&fs::read(&marker).unwrap()).unwrap();
    let start = std::time::Instant::now();
    let mut outcome = serde_json::json!({"mode":mode,"blocked":blocked});
    if mode == "setup-drop" {
        operation.abort();
        assert!(operation.await.unwrap_err().is_cancelled());
        tokio::time::timeout(Duration::from_secs(15), client.idle())
            .await
            .unwrap()
            .unwrap();
    } else {
        if mode.ends_with("cancel") {
            cancel.send_replace(true);
        }
        let result = tokio::time::timeout(Duration::from_secs(85), operation)
            .await
            .unwrap()
            .unwrap();
        if mode.ends_with("cancel") {
            assert!(result.unwrap().is_none());
        } else if mode == "cleanup-accepted" {
            let disposition = result.unwrap().unwrap().unwrap();
            assert!(
                disposition.published && !disposition.uncertain && disposition.recovery.is_some()
            );
            outcome["disposition"] = serde_json::to_value(disposition).unwrap();
        } else {
            let error = result.unwrap_err().to_string();
            assert!(error.contains("do not retry automatically"), "{error}");
            outcome["error"] = error.into();
        }
    }
    client.shutdown().await;
    if mode.starts_with("cleanup") {
        assert_eq!(
            fs::read(&target).unwrap(),
            b"%PDF-1.7\nworker fallback fixture"
        );
        let work = fs::read_dir(folder.path())
            .unwrap()
            .map(Result::unwrap)
            .find(|entry| {
                entry
                    .file_name()
                    .as_encoded_bytes()
                    .starts_with(b".canopa-print-")
            })
            .unwrap();
        assert_eq!(fs::read(work.path().join("previous")).unwrap(), b"original");
        outcome["staged_and_original_retained"] = true.into();
    } else {
        assert_eq!(fs::read(&target).unwrap(), b"original");
    }
    outcome["seconds_after_block"] = start.elapsed().as_secs_f64().into();
    outcome["status"] = "passed".into();
    fs::write(output, serde_json::to_vec_pretty(&outcome).unwrap()).unwrap();
}
