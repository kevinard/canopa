use super::super::{Client, fixture_document};
use super::*;
use crate::{
    print::{
        Format,
        file_target::{Pinned, Reviewed},
    },
    process::DialogProcess,
};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::Path,
    time::{Duration, Instant},
};

fn pin(path: &Path) -> Pinned {
    let reviewed = Reviewed::open(path).unwrap();
    let (send, receive) = UnixStream::pair().unwrap();
    reviewed.send(&send).unwrap();
    drop(send);
    Pinned::receive(&receive, reviewed.selection().clone()).unwrap()
}

#[tokio::test]
async fn parent_cleanup_requires_durable_job_identity() {
    // The stand-in worker models terminal failure and missing replies. The
    // assertions exercise the real parent subprocess/reap/cleanup path; they do
    // not establish scheduler-side identity or cancellation acceptance.
    for registered in [false, true] {
        for outcome in ["failed", "lost-final", "lost-create", "accepted"] {
            let folder = tempfile::tempdir().unwrap();
            let trace = folder.path().join("operations");
            let wrapper = folder.path().join("worker.py");
            fs::write(
                &wrapper,
                format!(
                    r#"#!/usr/bin/python3
import json,sys
operation=json.loads(sys.stdin.buffer.readline())['operation']
with open({trace:?},'a') as output:
    output.write(json.dumps(operation)+'\n')
def event(value):
    print(json.dumps(value),flush=True)
if operation['operation']=='submit':
    if {outcome:?}!='lost-create': event({{'event':'created','job':17}})
    if {outcome:?}=='accepted': event({{'event':'accepted','job':17}})
    elif {outcome:?}=='failed': event({{'event':'failed','message':'Fixture submission failed'}})
    else: sys.exit(2)
elif operation['operation']=='recover_printer_jobs':
    event({{'event':'retired_printer_jobs','report':{{'retired':0,'unresolved':1,'busy':0}}}})
elif operation['operation']=='cancel_job':
    event({{'event':'retired_job','job':operation['job']}})
else: sys.exit(3)
"#,
                    trace = trace.to_str().unwrap(),
                ),
            )
            .unwrap();
            fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
            let mut client = Client::new(DialogProcess {
                executable: wrapper,
                arguments: Vec::new(),
            });
            if registered {
                client = client.with_recovery(
                    crate::print::file_target::RecoveryStore::new(&folder.path().join("catalog"))
                        .unwrap(),
                );
            }
            let (_cancel, cancelled) = watch::channel(false);
            let document = fixture_document(b"%PDF-1.7\nparent cleanup fixture");
            let result = tokio::time::timeout(
                Duration::from_secs(10),
                client.submit(
                    &document,
                    super::super::super::Destination {
                        queue: "Fixture".into(),
                        instance: None,
                    },
                    "Parent cleanup fixture".into(),
                    Default::default(),
                    cancelled,
                ),
            )
            .await
            .unwrap();
            if outcome == "accepted" {
                assert_eq!(result.unwrap(), Some(17));
            } else {
                assert!(result.is_err(), "{registered}/{outcome}");
            }
            client.idle().await.unwrap();
            assert_eq!(client.shutdown().await, 0);
            let operations = fs::read_to_string(&trace)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                .collect::<Vec<_>>();
            let expected = if registered && outcome != "accepted" {
                vec!["submit", "recover_printer_jobs"]
            } else {
                vec!["submit"]
            };
            assert_eq!(
                operations
                    .iter()
                    .map(|operation| operation["operation"].as_str().unwrap())
                    .collect::<Vec<_>>(),
                expected,
                "{registered}/{outcome}",
            );
            if operations.len() == 2 {
                assert_eq!(operations[1]["interactive"], false);
            }
        }
    }
}

#[tokio::test]
async fn terminal_reply_wins_a_race_with_a_conservative_pending_notice() {
    let (_shutdown, shutdown) = watch::channel(false);
    let (reporter, notices) = Reporter::new(shutdown);
    let (reply, result) = oneshot::channel();
    reporter.pending(None, None);
    reply.send(Ok(Some(42))).unwrap();
    assert_eq!(
        receive(result, notices, |_| panic!(
            "pending notice replaced a terminal reply"
        ))
        .await
        .unwrap(),
        Some(42)
    );
}

#[tokio::test]
async fn pending_retirement_keeps_both_slots_and_shutdown_remains_bounded() {
    let folder = tempfile::tempdir().unwrap();
    let marker = folder.path().join("starts");
    let mut client = Client::new(DialogProcess { executable: "/bin/sh".into(), arguments: vec![
        "-c".into(), "read -r input; printf 'started\\n' >> \"$0\"; printf '%s\\n' '{\"event\":\"printers\",\"printers\":[]}'".into(), marker.clone().into_os_string()] });
    let gate = ReapGate::new(|_| true);
    client.reap_gate = Some(gate.clone());
    let (_cancel, cancelled) = watch::channel(false);
    let (a, b) = tokio::time::timeout(Duration::from_secs(12), async {
        tokio::join!(
            client.printers(cancelled.clone()),
            client.printers(cancelled.clone())
        )
    })
    .await
    .unwrap();
    assert!(a.unwrap_err().to_string().contains("retirement"));
    assert!(b.unwrap_err().to_string().contains("retirement"));
    assert_eq!(client.operations.available_permits(), 0);
    assert!(client.printers(cancelled.clone()).await.is_err());
    assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 2);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(20), client.shutdown())
            .await
            .unwrap(),
        2
    );
    assert_eq!(client.operations.available_permits(), 0);
    assert!(client.printers(cancelled).await.is_err());
    assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 2);
    gate.release();
    client.idle().await.unwrap();
    assert_eq!(client.shutdown().await, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "normal executable plus controlled exit-confirmation barrier; not a kernel D-state simulation"]
async fn native_retirement() {
    let binary = std::env::var_os("CANOPA_PRINT_RETIRE_BINARY").unwrap();
    let output = std::env::var_os("CANOPA_PRINT_RETIRE_OUTPUT").unwrap();
    let mode = std::env::var("CANOPA_PRINT_RETIRE_MODE").unwrap();
    assert!(
        [
            "snapshot",
            "publication-accepted",
            "publication-lost-receipt"
        ]
        .contains(&mode.as_str())
    );
    let folder = tempfile::tempdir().unwrap();
    let trace = folder.path().join("operations");
    let wrapper = folder.path().join("worker.py");
    fs::write(&wrapper, format!(r#"#!/usr/bin/python3
import json,os,subprocess,sys
raw=sys.stdin.buffer.readline();request=json.loads(raw);op=request['operation']
with open({trace:?},'a') as out: out.write(op['operation']+'\n')
if {mode:?}=='publication-lost-receipt' and op['operation']=='publish_file':
    fds=list(set([request['cancel'],op['document'],*op['descriptors']]))
    child=subprocess.run([{binary:?},'--print-worker'],input=raw,stdout=subprocess.PIPE,pass_fds=fds)
    assert child.returncode==0 and json.loads(child.stdout)['event']=='published_file'
    sys.exit(2)
fd=os.memfd_create('worker-input',os.MFD_CLOEXEC);os.write(fd,raw);os.lseek(fd,0,os.SEEK_SET);os.dup2(fd,0);os.close(fd)
os.execv({binary:?},[{binary:?},'--print-worker'])
"#,trace=trace.to_str().unwrap(),mode=mode,binary=binary.to_str().unwrap())).unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    let mut client = Client::new(DialogProcess {
        executable: wrapper,
        arguments: Vec::new(),
    });
    let gate = if mode == "snapshot" {
        ReapGate::new(|op| matches!(op, Operation::Snapshot { .. }))
    } else {
        ReapGate::new(|op| matches!(op, Operation::PublishFile { .. }))
    };
    client.reap_gate = Some(gate.clone());
    let (_cancel, cancelled) = watch::channel(false);
    let started = Instant::now();
    let mut evidence = serde_json::json!({"mode":mode,"scope":"real worker with delayed exit confirmation; OS child can already be terminal"});
    if mode == "snapshot" {
        let mut source = tempfile::tempfile().unwrap();
        source.write_all(b"%PDF-1.7\nquarantined source").unwrap();
        source.seek(SeekFrom::Start(0)).unwrap();
        let (a, b) = tokio::time::timeout(Duration::from_secs(12), async {
            tokio::join!(
                client.snapshot(source.try_clone().unwrap().into(), cancelled.clone()),
                client.snapshot(source.try_clone().unwrap().into(), cancelled.clone())
            )
        })
        .await
        .unwrap();
        assert!(a.is_err() && b.is_err());
        assert_eq!(client.documents.available_permits(), 0);
        assert_eq!(client.operations.available_permits(), 0);
        assert!(
            client
                .snapshot(source.try_clone().unwrap().into(), cancelled)
                .await
                .is_err()
        );
        assert_eq!(fs::read_to_string(&trace).unwrap().lines().count(), 2);
        evidence["reply_seconds"] = started.elapsed().as_secs_f64().into();
        let shutdown = Instant::now();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(20), client.shutdown())
                .await
                .unwrap(),
            2
        );
        evidence["shutdown_seconds"] = shutdown.elapsed().as_secs_f64().into();
        assert_eq!(client.documents.available_permits(), 0);
        assert_eq!(client.operations.available_permits(), 0);
        gate.release();
        client.idle().await.unwrap();
        assert_eq!(client.documents.available_permits(), 2);
        assert_eq!(client.operations.available_permits(), 2);
        assert_eq!(fs::read_to_string(&trace).unwrap().lines().count(), 2);
        evidence["storage_and_admission_held_until_confirmation"] = true.into();
    } else {
        let target = folder.path().join("output.pdf");
        fs::write(&target, b"original").unwrap();
        let frozen = fixture_document(b"%PDF-1.7\nretirement fixture");
        let result = tokio::time::timeout(
            Duration::from_secs(12),
            client.publish_file(&frozen, pin(&target), Format::Pdf, cancelled),
        )
        .await
        .unwrap();
        if mode == "publication-accepted" {
            let disposition = result.unwrap().unwrap();
            assert!(
                disposition.published && !disposition.uncertain && disposition.recovery.is_some()
            );
            evidence["disposition"] = serde_json::to_value(disposition).unwrap();
        } else {
            let error = result.unwrap_err().to_string();
            assert!(error.contains("do not retry automatically"));
            evidence["error"] = error.into();
        }
        evidence["reply_seconds"] = started.elapsed().as_secs_f64().into();
        assert_eq!(fs::read(&target).unwrap(), b"%PDF-1.7\nretirement fixture");
        let work = fs::read_dir(folder.path())
            .unwrap()
            .map(Result::unwrap)
            .find(|e| {
                e.file_name()
                    .as_encoded_bytes()
                    .starts_with(b".canopa-print-")
            })
            .unwrap();
        assert_eq!(
            fs::read(work.path().join("document")).unwrap(),
            b"%PDF-1.7\nretirement fixture"
        );
        assert_eq!(fs::read(work.path().join("previous")).unwrap(), b"original");
        let before = fs::read_to_string(&trace).unwrap();
        assert_eq!(
            before.lines().collect::<Vec<_>>(),
            ["prepare_file", "publish_file"]
        );
        assert_eq!(client.operations.available_permits(), 1);
        let shutdown = Instant::now();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(20), client.shutdown())
                .await
                .unwrap(),
            1
        );
        evidence["shutdown_seconds"] = shutdown.elapsed().as_secs_f64().into();
        assert_eq!(client.operations.available_permits(), 1);
        assert_eq!(fs::read_to_string(&trace).unwrap(), before);
        gate.release();
        client.idle().await.unwrap();
        assert_eq!(
            fs::read_to_string(&trace)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["prepare_file", "publish_file", "settle_file"]
        );
        assert!(!work.path().exists());
        assert_eq!(fs::read(&target).unwrap(), b"%PDF-1.7\nretirement fixture");
        evidence["no_settlement_before_confirmation"] = true.into();
        evidence["eventual_cleanup_after_shutdown"] = true.into();
    }
    assert_eq!(client.shutdown().await, 0);
    evidence["status"] = "passed".into();
    fs::write(output, serde_json::to_vec_pretty(&evidence).unwrap()).unwrap();
}
