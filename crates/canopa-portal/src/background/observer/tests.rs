use super::super::Activity;
use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
};

fn window(id: u64, pid: Option<i32>, focused: bool) -> WireWindow {
    WireWindow {
        id,
        pid,
        is_focused: focused,
    }
}
fn publisher() -> Publisher {
    let (state, _) = watch::channel(Observation::unavailable(Unavailable::Connecting));
    Publisher {
        revision: Mutex::new(0),
        closed: AtomicBool::new(false),
        state,
    }
}
fn empty_job(revision: u64) -> Job {
    Job {
        revision,
        windows: Vec::new(),
        queued: Instant::now(),
    }
}

#[test]
fn snapshot_is_required_and_window_incarnations_survive_only_their_original_process() {
    let mut windows = Windows {
        epoch: 1,
        ..Default::default()
    };
    assert_eq!(
        windows.apply(Event::WindowFocusChanged { id: None }),
        Err(Unavailable::InvalidInput)
    );
    windows
        .apply(Event::WindowsChanged {
            windows: vec![window(1, Some(12), true)],
        })
        .unwrap();
    let original = windows.windows[&1].key;
    assert!(
        !windows
            .apply(Event::WindowOpenedOrChanged {
                window: window(1, Some(12), true)
            })
            .unwrap()
    );
    assert_eq!(windows.windows[&1].key, original);
    windows
        .apply(Event::WindowOpenedOrChanged {
            window: window(1, Some(13), true),
        })
        .unwrap();
    assert_ne!(windows.windows[&1].key, original);
    let changed = windows.windows[&1].key;
    windows.apply(Event::WindowClosed { id: 1 }).unwrap();
    windows
        .apply(Event::WindowOpenedOrChanged {
            window: window(1, Some(13), true),
        })
        .unwrap();
    assert_ne!(windows.windows[&1].key, changed);
    assert_eq!(
        windows.apply(Event::WindowFocusChanged { id: Some(99) }),
        Err(Unavailable::InvalidInput)
    );
    windows
        .apply(Event::WindowFocusChanged { id: None })
        .unwrap();
    assert!(!windows.windows[&1].focused);
}

#[test]
fn snapshot_bounds_reject_duplicates_inconsistent_focus_and_invalid_processes() {
    for input in [
        vec![window(1, None, false), window(1, None, false)],
        vec![window(1, None, true), window(2, None, true)],
        vec![window(1, Some(0), false)],
    ] {
        assert_eq!(
            Windows::default().apply(Event::WindowsChanged { windows: input }),
            Err(Unavailable::InvalidInput)
        );
    }
    let mut windows = Windows::default();
    let full: Vec<_> = (0..MAX_WINDOWS as u64)
        .map(|id| window(id, None, false))
        .collect();
    windows
        .apply(Event::WindowsChanged { windows: full })
        .unwrap();
    assert_eq!(
        windows.apply(Event::WindowOpenedOrChanged {
            window: window(999, None, false)
        }),
        Err(Unavailable::Capacity)
    );
    let too_many = (0..=MAX_WINDOWS as u64)
        .map(|id| window(id, None, false))
        .collect();
    assert_eq!(
        windows.apply(Event::WindowsChanged { windows: too_many }),
        Err(Unavailable::Capacity)
    );
}

#[test]
fn unknown_events_are_ignored_but_malformed_known_events_retire_the_connection() {
    assert!(
        event(br#"{"NewNiriEvent":{"field":42}}"#)
            .unwrap()
            .is_none()
    );
    for line in [
        br#"{"WindowClosed":{"id":"wrong"}}"#.as_slice(),
        br#"{"WindowsChanged":{}}"#,
        b"{}",
        b"null",
        br#"{"WindowClosed":{"id":1},"Other":{}}"#,
    ] {
        assert!(matches!(event(line), Err(Unavailable::InvalidInput)));
    }
    // A declared app ID cannot enter the projection's identity data.
    let Some(Event::WindowOpenedOrChanged { window }) = event(br#"{"WindowOpenedOrChanged":{"window":{"id":1,"pid":7,"is_focused":true,"app_id":"org.spoofed.Other"}}}"#).unwrap() else { panic!("expected window event"); };
    assert_eq!(window.pid, Some(7));
}

#[test]
fn retired_or_slow_worker_results_cannot_publish_after_change_disconnect_or_shutdown() {
    let output = publisher();
    let first = output.invalidate(Unavailable::Updating);
    let old = empty_job(first);
    let current = output.invalidate(Unavailable::Disconnected);
    output.publish(&old, Ok(AppStates::new()));
    assert_eq!(output.state.borrow().apps(), Err(Unavailable::Disconnected));
    let mut slow = empty_job(current);
    slow.queued = Instant::now() - FRESHNESS - Duration::from_millis(1);
    output.publish(&slow, Ok(AppStates::new()));
    assert_eq!(output.state.borrow().apps(), Err(Unavailable::Stale));
    output.close();
    output.invalidate(Unavailable::Updating);
    output.publish(&empty_job(current), Ok(AppStates::new()));
    assert_eq!(output.state.borrow().apps(), Err(Unavailable::Shutdown));
}

#[test]
fn refresh_updates_freshness_without_repeating_an_unchanged_application_signal() {
    let output = publisher();
    let revision = output.invalidate(Unavailable::Updating);
    let apps = AppStates::from([("org.canopa.Fixture".into(), Activity::Active)]);
    output.publish(&empty_job(revision), Ok(apps.clone()));
    let mut observed = output.state.subscribe();
    output.publish(&empty_job(revision), Ok(apps));
    assert!(!observed.has_changed().unwrap());
    output.publish(&empty_job(revision), Ok(AppStates::new()));
    assert!(observed.has_changed().unwrap());
    observed.borrow_and_update();
    let mut stale = Observation::available(AppStates::new());
    stale.at = Instant::now() - FRESHNESS - Duration::from_millis(1);
    assert_eq!(stale.apps(), Err(Unavailable::Stale));
    output.state.send_replace(stale);
    observed.borrow_and_update();
    output.expire();
    assert!(observed.has_changed().unwrap());
    assert_eq!(observed.borrow().apps(), Err(Unavailable::Stale));
}

#[test]
fn worker_inbox_coalesces_and_shutdown_retires_pending_work() {
    let inbox = Inbox::default();
    for revision in 1..100 {
        inbox.submit(empty_job(revision));
    }
    assert_eq!(inbox.next().unwrap().revision, 99);
    inbox.submit(empty_job(100));
    inbox.close();
    assert!(inbox.next().is_none());
    inbox.submit(empty_job(101));
    assert!(inbox.next().is_none());
}

#[tokio::test]
async fn line_budget_and_cancellation_keep_partial_input_bounded_and_intact() {
    let (mut writer, stream) = tokio::io::duplex(64);
    let mut reader = BufReader::new(stream);
    let mut bytes = Vec::new();
    let mut started = None;
    writer.write_all(b"{").await.unwrap();
    assert!(
        tokio::time::timeout(
            Duration::from_millis(20),
            line(&mut reader, &mut bytes, &mut started)
        )
        .await
        .is_err()
    );
    assert_eq!(bytes, b"{");
    assert!(started.is_some());
    writer.write_all(b"}\n").await.unwrap();
    line(&mut reader, &mut bytes, &mut started).await.unwrap();
    assert_eq!(bytes, b"{}\n");
    let mut large = BufReader::new(std::io::Cursor::new(vec![b'x'; LINE_BYTES + 1]));
    assert!(line(&mut large, &mut Vec::new(), &mut None).await.is_err());
}

async fn available(observed: &mut watch::Receiver<Observation>) {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if observed.borrow_and_update().apps().is_ok() {
                return;
            }
            observed.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn socket_observation_requires_a_snapshot_and_drop_closes_the_connection() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("niri.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let observer = Observer::spawn(Some(path)).unwrap();
    let mut observed = observer.subscribe();
    let (stream, _) = listener.accept().await.unwrap();
    let mut stream = BufReader::new(stream);
    let mut request = String::new();
    stream.read_line(&mut request).await.unwrap();
    assert_eq!(request, "\"EventStream\"\n");
    stream
        .get_mut()
        .write_all(b"{\"Ok\":\"Handled\"}\n{\"Future\":{}}\n")
        .await
        .unwrap();
    assert!(observed.borrow().apps().is_err());
    stream
        .get_mut()
        .write_all(b"{\"WindowsChanged\":{\"windows\":[]}}\n")
        .await
        .unwrap();
    available(&mut observed).await;
    assert!(observed.borrow().apps().unwrap().is_empty());
    drop(observer);
    assert_eq!(observed.borrow().apps(), Err(Unavailable::Shutdown));
    let mut byte = [0];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), stream.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn reconnect_retires_old_identity_failure_and_reads_a_new_complete_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("niri.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let observer = Observer::spawn(Some(path)).unwrap();
    let mut observed = observer.subscribe();
    let (mut first, _) = listener.accept().await.unwrap();
    first.write_all(b"{\"Ok\":\"Handled\"}\n{\"WindowsChanged\":{\"windows\":[{\"id\":1,\"pid\":null,\"is_focused\":false}]}}\n").await.unwrap();
    drop(first);
    let (mut second, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
        .await
        .unwrap()
        .unwrap();
    second
        .write_all(b"{\"Ok\":\"Handled\"}\n{\"WindowsChanged\":{\"windows\":[]}}\n")
        .await
        .unwrap();
    available(&mut observed).await;
    assert!(observed.borrow().apps().unwrap().is_empty());
}

#[tokio::test]
async fn a_partial_event_cannot_keep_an_old_snapshot_available_forever() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("niri.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let observer = Observer::spawn(Some(path)).unwrap();
    let mut observed = observer.subscribe();
    let (mut stream, _) = listener.accept().await.unwrap();
    stream
        .write_all(b"{\"Ok\":\"Handled\"}\n{\"WindowsChanged\":{\"windows\":[]}}\n")
        .await
        .unwrap();
    available(&mut observed).await;
    stream.write_all(b"{").await.unwrap();
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if observed.borrow_and_update().apps().is_err() {
                return;
            }
            observed.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        observed.borrow().apps(),
        Err(Unavailable::InvalidInput | Unavailable::Disconnected)
    ));
}
