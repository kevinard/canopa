//! Actual worker exec/lease lifecycle against a private compositor contract.
#[path = "../src/screencast_capture/tests/fake.rs"]
mod fake;
use canopa_portal::{
    process::DialogProcess,
    screencast_capture::{Cursor, Owner, Recorder, Source},
};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;

fn recorder(fixture: &fake::Fixture) -> Recorder {
    Recorder::new(Arc::new(DialogProcess {
        executable: "/usr/bin/env".into(),
        arguments: vec![
            format!("DBUS_SESSION_BUS_ADDRESS={}", fixture.bus.address).into(),
            env!("CARGO_BIN_EXE_xdg-desktop-portal-canopa").into(),
        ],
    }))
}

#[tokio::test]
async fn real_worker_lease_drop_stop_and_caller_loss_retire_streams() {
    let fixture = fake::Fixture::new().await;
    let recorder = recorder(&fixture);
    let owner = Owner::resolve(&fixture.backend).await.unwrap();
    let (cancel, cancelled) = watch::channel(false);
    let recording = tokio::time::timeout(
        Duration::from_secs(5),
        recorder.start(
            &fixture.backend,
            &owner,
            fixture.caller.unique_name().unwrap().to_owned(),
            vec![Source::Monitor("DP-1".into()), Source::Window(42)],
            Cursor::Embedded,
            cancelled,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(recording.streams().len(), 2);
    drop(cancel); // Finishing Start must not end the recording.
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert!(!*recording.closed().borrow());
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    tokio::time::timeout(Duration::from_secs(5), recording.stop())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );

    let (_cancel, cancelled) = watch::channel(false);
    let recording = recorder
        .start(
            &fixture.backend,
            &owner,
            fixture.caller.unique_name().unwrap().to_owned(),
            vec![Source::Window(42)],
            Cursor::Hidden,
            cancelled,
        )
        .await
        .unwrap();
    let mut closed = recording.closed();
    drop(recording);
    tokio::time::timeout(Duration::from_secs(5), async {
        while !*closed.borrow() {
            closed.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        2
    );

    let (_cancel, cancelled) = watch::channel(false);
    let recording = recorder
        .start(
            &fixture.backend,
            &owner,
            fixture.caller.unique_name().unwrap().to_owned(),
            vec![Source::Window(42)],
            Cursor::Metadata,
            cancelled,
        )
        .await
        .unwrap();
    let mut closed = recording.closed();
    fixture.caller.clone().close().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !*closed.borrow() {
            closed.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    recording.stop().await.unwrap();
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        3
    );
}

#[tokio::test]
async fn killing_the_backend_process_leaves_worker_alive_long_enough_to_stop() {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let fixture = fake::Fixture::new().await;
    let program = r#"import gi,json,os,signal,subprocess,sys
from gi.repository import Gio
flags=Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT|Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION
connection=Gio.DBusConnection.new_for_address_sync(os.environ['DBUS_SESSION_BUS_ADDRESS'],flags,None,None)
worker=subprocess.Popen([sys.argv[1],'--screencast-worker'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
owner=connection.get_unique_name()
worker.stdin.write(json.dumps({'compositor':sys.argv[2],'backend':owner,'caller':owner,'sources':[{'Window':42}],'cursor':'Hidden'})+'\n');worker.stdin.flush()
assert json.loads(worker.stdout.readline())[0]['node']==101
print(worker.pid,flush=True)
signal.pause()
"#;
    let mut backend = tokio::process::Command::new("/usr/bin/python3")
        .args([
            "-c",
            program,
            env!("CARGO_BIN_EXE_xdg-desktop-portal-canopa"),
            fixture.service.unique_name().unwrap().as_str(),
        ])
        .env("DBUS_SESSION_BUS_ADDRESS", &fixture.bus.address)
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut line = String::new();
    tokio::time::timeout(
        Duration::from_secs(5),
        BufReader::new(backend.stdout.take().unwrap()).read_line(&mut line),
    )
    .await
    .unwrap()
    .unwrap();
    let worker: u32 = line.trim().parse().unwrap();
    backend.kill().await.unwrap(); // SIGKILL: no backend destructor can run.
    tokio::time::timeout(Duration::from_secs(5), async {
        while fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst)
            == 0
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        loop {
            match std::fs::read_to_string(format!("/proc/{worker}/stat")) {
                Err(_) => break,
                Ok(stat) if stat.rsplit_once(") ").unwrap().1.starts_with('Z') => break,
                _ => tokio::time::sleep(Duration::from_millis(10)).await,
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn worker_admission_is_released_only_after_retirement() {
    let fixture = fake::Fixture::new().await;
    let recorder = recorder(&fixture);
    let owner = Owner::resolve(&fixture.backend).await.unwrap();
    let (_cancel, cancelled) = watch::channel(false);
    let mut recordings = Vec::new();
    for id in 1..=8 {
        recordings.push(
            recorder
                .start(
                    &fixture.backend,
                    &owner,
                    fixture.caller.unique_name().unwrap().to_owned(),
                    vec![Source::Window(id)],
                    Cursor::Hidden,
                    cancelled.clone(),
                )
                .await
                .unwrap(),
        );
    }
    assert!(
        recorder
            .start(
                &fixture.backend,
                &owner,
                fixture.caller.unique_name().unwrap().to_owned(),
                vec![Source::Window(9)],
                Cursor::Hidden,
                cancelled.clone()
            )
            .await
            .is_err()
    );
    tokio::time::timeout(Duration::from_secs(5), recordings.remove(0).stop())
        .await
        .unwrap()
        .unwrap();
    recordings.push(
        recorder
            .start(
                &fixture.backend,
                &owner,
                fixture.caller.unique_name().unwrap().to_owned(),
                vec![Source::Window(9)],
                Cursor::Hidden,
                cancelled.clone(),
            )
            .await
            .unwrap(),
    );
    for recording in recordings {
        tokio::time::timeout(Duration::from_secs(5), recording.stop())
            .await
            .unwrap()
            .unwrap();
    }
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        9
    );
}

#[tokio::test]
async fn multiple_recordings_share_one_total_source_budget() {
    let fixture = fake::Fixture::new().await;
    let recorder = recorder(&fixture);
    let owner = Owner::resolve(&fixture.backend).await.unwrap();
    let (_cancel, cancelled) = watch::channel(false);
    let first = recorder
        .start(
            &fixture.backend,
            &owner,
            fixture.caller.unique_name().unwrap().to_owned(),
            (1..=8).map(Source::Window).collect(),
            Cursor::Hidden,
            cancelled.clone(),
        )
        .await
        .unwrap();
    let second = recorder
        .start(
            &fixture.backend,
            &owner,
            fixture.caller.unique_name().unwrap().to_owned(),
            (9..=16).map(Source::Window).collect(),
            Cursor::Hidden,
            cancelled.clone(),
        )
        .await
        .unwrap();
    assert!(
        recorder
            .start(
                &fixture.backend,
                &owner,
                fixture.caller.unique_name().unwrap().to_owned(),
                vec![Source::Window(17)],
                Cursor::Hidden,
                cancelled.clone()
            )
            .await
            .is_err()
    );
    first.stop().await.unwrap();
    let replacement = recorder
        .start(
            &fixture.backend,
            &owner,
            fixture.caller.unique_name().unwrap().to_owned(),
            vec![Source::Window(17)],
            Cursor::Hidden,
            cancelled.clone(),
        )
        .await
        .unwrap();
    second.stop().await.unwrap();
    replacement.stop().await.unwrap();
    assert_eq!(
        fixture
            .state
            .stops
            .load(std::sync::atomic::Ordering::SeqCst),
        3
    );
}

#[tokio::test]
async fn real_worker_preserves_monitor_identity_and_rejects_same_name_replacement() {
    use canopa_portal::screencast_capture::Monitor;
    use std::sync::atomic::Ordering;
    let fixture = fake::Fixture::new().await;
    fixture.state.source_version.store(1, Ordering::SeqCst);
    fixture.state.monitor_identity.store(77, Ordering::SeqCst);
    let recorder = recorder(&fixture);
    let owner = Owner::resolve(&fixture.backend).await.unwrap();
    let source = Source::Monitor(Monitor::pinned("DP-1", 77).unwrap());
    let (_cancel, cancelled) = watch::channel(false);
    let recording = recorder
        .start(
            &fixture.backend,
            &owner,
            fixture.caller.unique_name().unwrap().to_owned(),
            vec![source.clone()],
            Cursor::Hidden,
            cancelled.clone(),
        )
        .await
        .unwrap();
    assert_eq!(recording.streams()[0].source, source);
    assert!(recording.bindings().is_some());
    assert_eq!(
        *fixture.state.identity_preconditions.lock().unwrap(),
        [Some(77)]
    );
    recording.stop().await.unwrap();
    fixture.state.monitor_identity.store(78, Ordering::SeqCst);
    assert!(
        recorder
            .start(
                &fixture.backend,
                &owner,
                fixture.caller.unique_name().unwrap().to_owned(),
                vec![source],
                Cursor::Hidden,
                cancelled
            )
            .await
            .is_err()
    );
    assert_eq!(fixture.state.starts.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.state.stops.load(Ordering::SeqCst), 2);
}
