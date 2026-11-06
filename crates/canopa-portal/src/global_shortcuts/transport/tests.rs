use super::*;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[path = "../session_tests.rs"]
mod portal;

async fn send(stream: &mut UnixStream, value: Value) {
    let payload = serde_json::to_vec(&value).unwrap();
    stream.write_u32_le(payload.len() as u32).await.unwrap();
    stream.write_all(&payload).await.unwrap();
}
async fn request(stream: &mut UnixStream) -> Value {
    tokio::time::timeout(Duration::from_secs(2), async {
        let len = stream.read_u32_le().await.unwrap();
        assert!(len > 0 && len <= wire::MAX_FRAME as u32);
        let mut payload = vec![0; len as usize];
        stream.read_exact(&mut payload).await.unwrap();
        serde_json::from_slice(&payload).unwrap()
    })
    .await
    .unwrap()
}
fn reply(sequence: u64, generation: u64, bindings: Value, error: Value, failed: Value) -> Value {
    json!({"event":"reply","sequence":sequence,"generation":generation,"bindings":bindings,"error":error,"failed":failed})
}
async fn ready_with(owners: BoxStream<'static, Option<String>>) -> (Lease, UnixStream) {
    let (client, mut server) = UnixStream::pair().unwrap();
    let opening = tokio::spawn(initialize(client, owners, ":1.42".into()));
    assert_eq!(request(&mut server).await["command"]["op"], "list");
    send(&mut server, reply(1, 0, json!([]), Value::Null, json!([]))).await;
    (opening.await.unwrap().unwrap(), server)
}
async fn ready() -> (Lease, UnixStream) {
    ready_with(futures_util::stream::pending().boxed()).await
}
async fn closed(mut state: watch::Receiver<bool>) {
    tokio::time::timeout(Duration::from_secs(2), async { ended(&mut state).await })
        .await
        .unwrap();
}
async fn peer_ended(server: &mut UnixStream) {
    let mut byte = [0];
    let result = tokio::time::timeout(Duration::from_secs(2), server.read(&mut byte))
        .await
        .unwrap();
    assert!(
        matches!(result, Ok(0))
            || matches!(result,Err(ref e) if e.kind()==std::io::ErrorKind::ConnectionReset),
        "{result:?}"
    );
}
async fn commit(
    control: Control,
    server: &mut UnixStream,
    expected: u64,
    generation: u64,
) -> Commit {
    let operation =
        tokio::spawn(async move { control.replace(expected, vec![(7, "F9".into())]).await });
    let request = request(server).await;
    assert_eq!(request["command"]["generation"], expected);
    send(
        server,
        reply(
            request["sequence"].as_u64().unwrap(),
            generation,
            json!([[7, "F9"]]),
            Value::Null,
            json!([]),
        ),
    )
    .await;
    operation.await.unwrap().unwrap()
}

#[tokio::test]
async fn fragmented_read_survives_an_interleaved_rpc_and_preserves_source_timestamp() {
    let (mut lease, mut server) = ready().await;
    let control = lease.control();
    let mut events = lease.events().unwrap();
    let payload =
        serde_json::to_vec(&json!({"event":"changed","generation":2,"bindings":[],"failed":[]}))
            .unwrap();
    let mut bytes = (payload.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&payload);
    server.write_all(&bytes[..2]).await.unwrap();
    let operation = {
        let control = control.clone();
        tokio::spawn(async move { control.replace(0, vec![(7, "F9".into())]).await })
    };
    let sent = request(&mut server).await;
    server.write_all(&bytes[2..]).await.unwrap();
    send(
        &mut server,
        reply(
            sent["sequence"].as_u64().unwrap(),
            2,
            json!([]),
            json!("stale_generation"),
            json!([]),
        ),
    )
    .await;
    assert_eq!(
        operation.await.unwrap().unwrap().error.as_deref(),
        Some("stale_generation")
    );
    assert_eq!(
        events.recv().await.unwrap(),
        Notice::Changed(Snapshot {
            generation: 2,
            bindings: BTreeMap::new()
        })
    );
    assert_eq!(
        commit(control.clone(), &mut server, 2, 3)
            .await
            .snapshot
            .generation,
        3
    );
    assert!(matches!(
        events.recv().await.unwrap(),
        Notice::Changed(Snapshot { generation: 3, .. })
    ));
    for (kind, time) in [
        ("activated", 5_000_000_123u64),
        ("deactivated", 5_000_000_124u64),
    ] {
        send(
            &mut server,
            json!({"event":kind,"generation":3,"binding":7,"timestamp_ms":time}),
        )
        .await;
    }
    assert!(matches!(
        events.recv().await.unwrap(),
        Notice::Activated {
            timestamp_ms: 5_000_000_123,
            ..
        }
    ));
    assert!(matches!(
        events.recv().await.unwrap(),
        Notice::Deactivated {
            timestamp_ms: 5_000_000_124,
            ..
        }
    ));
    lease.stop().await;
    peer_ended(&mut server).await;
}

#[tokio::test]
async fn dropping_owner_closes_fd_even_with_retained_controller() {
    let (lease, mut server) = ready().await;
    let control = lease.control();
    let state = control.closed();
    drop(lease);
    closed(state).await;
    peer_ended(&mut server).await;
    assert!(control.replace(0, vec![(7, "F9".into())]).await.is_err());
}

#[tokio::test]
async fn malformed_oversized_and_unapproved_events_close_authority() {
    for malformed in [
        None,
        Some(json!({"event":"activated","generation":0,"binding":7,"timestamp_ms":1})),
        Some(json!({"event":"changed","generation":1,"bindings":[[7,"F9"]],"failed":[]})),
        Some(json!({"event":"changed","generation":1,"bindings":[],"failed":[],"extra":true})),
    ] {
        let (lease, mut server) = ready().await;
        let state = lease.control().closed();
        if let Some(value) = malformed {
            send(&mut server, value).await;
        } else {
            server
                .write_u32_le(wire::MAX_FRAME as u32 + 1)
                .await
                .unwrap();
        }
        closed(state).await;
        peer_ended(&mut server).await;
    }
}

#[tokio::test]
async fn wrong_reply_sequence_cannot_acknowledge_another_mutation() {
    let (lease, mut server) = ready().await;
    let control = lease.control();
    let state = control.closed();
    let operation = tokio::spawn(async move { control.replace(0, vec![(7, "F9".into())]).await });
    request(&mut server).await;
    send(
        &mut server,
        reply(999, 1, json!([[7, "F9"]]), Value::Null, json!([])),
    )
    .await;
    assert!(operation.await.unwrap().is_err());
    closed(state).await;
    peer_ended(&mut server).await;
}

#[tokio::test]
async fn original_owner_loss_latches_and_late_reply_cannot_revive_the_lease() {
    let (owner, incoming) = mpsc::channel(4);
    let events = futures_util::stream::unfold(incoming, |mut stream| async move {
        stream.recv().await.map(|event| (event, stream))
    })
    .boxed();
    let (lease, mut server) = ready_with(events).await;
    let control = lease.control();
    let state = control.closed();
    let operation = tokio::spawn(async move { control.replace(0, vec![(7, "F9".into())]).await });
    request(&mut server).await;
    owner.send(Some(":1.43".into())).await.unwrap();
    closed(state).await;
    assert!(operation.await.unwrap().is_err());
    peer_ended(&mut server).await;
    assert!(owner.send(Some(":1.42".into())).await.is_err());
}

#[tokio::test]
async fn refused_mutation_preserves_authority_but_cannot_later_restore_an_unapproved_trigger() {
    let (mut lease, mut server) = ready().await;
    let control = lease.control();
    let mut events = lease.events().unwrap();
    let operation = {
        let control = control.clone();
        tokio::spawn(async move { control.replace(0, vec![(7, "F9".into())]).await })
    };
    let first = request(&mut server).await;
    send(
        &mut server,
        reply(
            first["sequence"].as_u64().unwrap(),
            0,
            json!([]),
            json!("conflict"),
            json!([7]),
        ),
    )
    .await;
    assert_eq!(
        operation.await.unwrap().unwrap().error.as_deref(),
        Some("conflict")
    );
    assert!(events.try_recv().is_err());
    let operation = {
        let control = control.clone();
        tokio::spawn(async move { control.replace(0, vec![(7, "F10".into())]).await })
    };
    let second = request(&mut server).await;
    send(
        &mut server,
        reply(
            second["sequence"].as_u64().unwrap(),
            1,
            json!([[7, "F10"]]),
            Value::Null,
            json!([]),
        ),
    )
    .await;
    operation.await.unwrap().unwrap();
    events.recv().await.unwrap();
    send(
        &mut server,
        json!({"event":"changed","generation":2,"bindings":[[7,"F9"]],"failed":[]}),
    )
    .await;
    closed(control.closed()).await;
    peer_ended(&mut server).await;
}

#[tokio::test]
async fn a_stalled_event_consumer_retires_the_descriptor_without_unbounded_buffering() {
    let (mut lease, mut server) = ready().await;
    let control = lease.control();
    let _unread = lease.events().unwrap();
    commit(control.clone(), &mut server, 0, 1).await;
    let mut bytes = Vec::new();
    for _ in 0..100 {
        for event in ["activated", "deactivated"] {
            let payload = serde_json::to_vec(
                &json!({"event":event,"generation":1,"binding":7,"timestamp_ms":42}),
            )
            .unwrap();
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&payload);
        }
    }
    let _ = server.write_all(&bytes).await;
    closed(control.closed()).await;
    peer_ended(&mut server).await;
}

#[tokio::test]
async fn dropped_mutation_recipient_cannot_leave_unaccounted_authority() {
    let (mut lease, mut server) = ready().await;
    let control = lease.control();
    let mut events = lease.events().unwrap();
    let operation = {
        let control = control.clone();
        tokio::spawn(async move { control.replace(0, vec![(7, "F9".into())]).await })
    };
    let sent = request(&mut server).await;
    operation.abort();
    let _ = operation.await;
    let payload = serde_json::to_vec(&reply(
        sent["sequence"].as_u64().unwrap(),
        1,
        json!([[7, "F9"]]),
        Value::Null,
        json!([]),
    ))
    .unwrap();
    let mut bytes = (payload.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&payload);
    let _ = server.write_all(&bytes).await;
    closed(control.closed()).await;
    peer_ended(&mut server).await;
    while let Ok(event) = events.try_recv() {
        assert!(!matches!(event, Notice::Activated { .. }));
    }
}

#[tokio::test]
async fn descriptor_origin_requires_unix_stream_and_original_process() {
    let pid = rustix::process::getpid().as_raw_nonzero().get() as u32;
    let (stream, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
    assert!(socket(stream.into(), pid + 1).is_err());
    assert!(socket(tempfile::tempfile().unwrap().into(), pid).is_err());
    let (stream, _peer) = std::os::unix::net::UnixStream::pair().unwrap();
    assert!(socket(stream.into(), pid).is_ok());
}

#[tokio::test]
#[ignore = "requires scripts/test-global-shortcuts-transport.py and its private compositor"]
async fn native_transport_fixture() {
    use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt, path::Path};
    let output = std::env::var_os("CANOPA_SHORTCUT_TRANSPORT_RECEIPT")
        .expect("private native fixture output");
    let runtime =
        std::env::var_os("CANOPA_SHORTCUT_TRANSPORT_RUNTIME").expect("private native runtime");
    let socket = PathBuf::from(std::env::var_os("NIRI_SOCKET").expect("private Niri socket"));
    assert!(
        socket.is_absolute() && socket.starts_with(Path::new(&runtime)),
        "Native fixture must use its private compositor"
    );
    let connection = Connection::session().await.unwrap();
    let endpoint = Endpoint::resolve(&connection, socket).await.unwrap();
    let mut lease = endpoint.open(&connection).await.unwrap();
    let control = lease.control();
    let mut events = lease.events().unwrap();
    let snapshot = control.snapshot();
    assert!(snapshot.bindings.is_empty());
    let committed = control
        .replace(
            snapshot.generation,
            vec![(1, "CTRL+a".into()), (2, "F9".into())],
        )
        .await
        .unwrap();
    assert!(committed.error.is_none());
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output)
        .unwrap();
    let ready = serde_json::json!({"event":"ready","generation":committed.snapshot.generation,"owner":endpoint.owner.as_str(),"pid":endpoint.pid});
    writeln!(file, "{ready}").unwrap();
    file.flush().unwrap();
    let mut closing = control.closed();
    tokio::time::timeout(Duration::from_secs(30),async {
        loop {
            let notice=tokio::select! {biased;
                _=ended(&mut closing)=>panic!("Native shortcut transport closed early"),
                event=events.recv()=>event.expect("native shortcut events"),
            };
            let value=match notice {
                Notice::Changed(snapshot)=>serde_json::json!({"event":"changed","generation":snapshot.generation,"bindings":snapshot.bindings}),
                Notice::Activated {generation,binding,timestamp_ms}=>serde_json::json!({"event":"activated","generation":generation,"binding":binding,"timestamp_ms":timestamp_ms}),
                Notice::Deactivated {generation,binding,timestamp_ms}=>serde_json::json!({"event":"deactivated","generation":generation,"binding":binding,"timestamp_ms":timestamp_ms}),
            };
            writeln!(file,"{value}").unwrap();file.flush().unwrap();
            if value["event"]=="activated"&&value["binding"]==2 {
                // The native driver now kills this exact process by pidfd.
                // Waiting is bounded so a manually invoked ignored test fails.
                tokio::time::sleep(Duration::from_secs(20)).await;
                panic!("Native fixture did not kill the transport owner");
            }
        }
    }).await.expect("native transport fixture timed out");
}
