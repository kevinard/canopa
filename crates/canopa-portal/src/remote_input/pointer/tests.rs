use super::*;

#[test]
fn pointer_wire_preserves_values_and_rejects_reserved_or_unbounded_input() {
    for command in [
        Command::Barrier,
        Command::Relative { x: -1.5, y: 2.25 },
        Command::Absolute {
            node: 101,
            x: 7.5,
            y: 8.,
        },
        Command::Button {
            code: 272,
            pressed: true,
        },
        Command::Scroll {
            x: Some(0.),
            y: Some(-1.25),
            finger: true,
        },
        Command::Wheel {
            x: None,
            y: Some(-120),
        },
    ] {
        assert_eq!(
            Command::decode(&command.packet(1, None), 1).unwrap(),
            command
        );
    }
    for command in [
        Command::Relative { x: f64::NAN, y: 0. },
        Command::Relative {
            x: 1_000_001.,
            y: 0.,
        },
        Command::Absolute {
            node: 0,
            x: 0.,
            y: 0.,
        },
        Command::Absolute {
            node: 101,
            x: -1.,
            y: 0.,
        },
        Command::Button {
            code: 0,
            pressed: true,
        },
        Command::Wheel {
            x: Some(0),
            y: None,
        },
        Command::Wheel {
            x: Some(i32::MIN),
            y: None,
        },
        Command::Scroll {
            x: None,
            y: None,
            finger: false,
        },
    ] {
        assert!(!command.valid());
        assert!(Command::decode(&command.packet(1, None), 1).is_err());
    }
    let mut bytes = Command::Barrier.packet(1, None);
    bytes[8] = 1;
    assert!(Command::decode(&bytes, 1).is_err());
    assert!(Command::decode(&Command::Barrier.packet(2, None), 1).is_err());
    assert!(wire::status(wire::feedback(2, Status::Accepted), 1).is_err());
    let mut reply = wire::feedback(1, Status::Accepted);
    reply[12] = 1;
    assert!(wire::status(reply, 1).is_err());
    let bytes = Command::Absolute {
        node: 101,
        x: 1.,
        y: 2.,
    }
    .packet(1, Some(u64::MAX));
    assert_eq!(
        u64::from_le_bytes(bytes[24..].try_into().unwrap()),
        u64::MAX
    );
}

#[tokio::test]
async fn dropping_a_pending_pointer_send_revokes_its_worker_instead_of_reusing_an_ambiguous_ack() {
    let (socket, mut peer) = tokio::net::UnixStream::pair().unwrap();
    let (release, released) = tokio::sync::oneshot::channel();
    let (_closed, closed) = watch::channel(false);
    let (_done, done) = tokio::sync::oneshot::channel();
    let lease = InputLease::fixture(release, closed, done);
    let mut input = PointerInput {
        socket,
        lease,
        sequence: 0,
        failed: false,
    };
    let (observed, received) = tokio::sync::oneshot::channel();
    let receiver = tokio::spawn(async move {
        let mut data = [0; 32];
        peer.read_exact(&mut data).await.unwrap();
        observed.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    let mut sending = Box::pin(input.send(Command::Button {
        code: 272,
        pressed: true,
    }));
    tokio::select! { result=&mut sending=>panic!("unexpected response: {result:?}"), _=received=>{} }
    drop(sending);
    // Revocation closes the lease channel rather than sending a value.
    assert!(released.await.is_err());
    assert!(*input.closed().borrow());
    assert!(input.send(Command::Barrier).await.is_err());
    receiver.abort();
}

#[tokio::test]
#[ignore = "requires the private pointer compositor/media fixture"]
async fn native_pointer_fixture() {
    use crate::{
        process::DialogProcess,
        screencast_capture::{Cursor, Owner, Source},
    };
    use std::{
        io::{BufRead, Write},
        sync::Arc,
    };
    assert_eq!(std::env::var("CANOPA_POINTER_NATIVE").as_deref(), Ok("1"));
    let process = Arc::new(DialogProcess {
        executable: std::env::var_os("CANOPA_PORTAL_BINARY").unwrap().into(),
        arguments: vec!["--defaults".into()],
    });
    let connection = Connection::session().await.unwrap();
    let caller = Connection::session().await.unwrap();
    let owner = Owner::resolve(&connection).await.unwrap();
    let mut sources = if std::env::var("CANOPA_POINTER_WINDOW_ONLY").as_deref() == Ok("1") {
        Vec::new()
    } else {
        let names = owner
            .monitors(&connection, vec!["winit".into()])
            .await
            .unwrap();
        vec![Source::Monitor(names.into_iter().next().unwrap())]
    };
    if let Ok(id) = std::env::var("CANOPA_POINTER_WINDOW") {
        sources.push(Source::Window(id.parse().unwrap()));
    }
    let recorder = crate::screencast::Production::new(process.clone());
    let (_cancel, cancelled) = watch::channel(false);
    let recording = recorder
        .start_approved(
            &connection,
            &owner,
            caller.unique_name().unwrap().to_owned(),
            sources,
            Cursor::Hidden,
            cancelled.clone(),
        )
        .await
        .unwrap();
    let binding = recording
        .input_bindings()
        .expect("live capture mappings")
        .clone();
    let endpoint = Endpoint::resolve(&connection, std::env::var_os("NIRI_SOCKET").unwrap().into())
        .await
        .unwrap();
    let connector = Connector::new(process);
    let mut pointer = Some(
        connector
            .start_pointer(
                &connection,
                &endpoint,
                caller.unique_name().unwrap().to_owned(),
                Some(&binding),
                cancelled.clone(),
            )
            .await
            .unwrap(),
    );
    let streams: Vec<_> = recording.streams().iter().map(|stream| serde_json::json!({
        "node":stream.node,"serial":stream.serial,"source":stream.source,"geometry":stream.geometry,
        "mapping":stream.mapping.as_ref().map(|mapping|serde_json::json!({"id":mapping.id,"size":mapping.size}))
    })).collect();
    println!(
        "POINTER {}",
        serde_json::json!({"ready":true,"streams":streams,"binding":&binding,"pid":std::process::id()})
    );
    std::io::stdout().flush().unwrap();
    let (send, mut receive) = tokio::sync::mpsc::channel(1);
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if send.blocking_send(line).is_err() {
                break;
            }
        }
    });
    while let Some(line) = receive.recv().await {
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        let result = if value.get("close") == Some(&serde_json::Value::Bool(true)) {
            pointer.take().unwrap().stop().await.unwrap();
            serde_json::json!({"closed":true})
        } else if value.get("reopen") == Some(&serde_json::Value::Bool(true)) {
            pointer = Some(
                connector
                    .start_pointer(
                        &connection,
                        &endpoint,
                        caller.unique_name().unwrap().to_owned(),
                        Some(&binding),
                        cancelled.clone(),
                    )
                    .await
                    .unwrap(),
            );
            serde_json::json!({"reopened":true})
        } else if value.get("state") == Some(&serde_json::Value::Bool(true)) {
            serde_json::json!({"closed":pointer.as_ref().is_none_or(|p|*p.closed().borrow())})
        } else if value.get("stop") == Some(&serde_json::Value::Bool(true)) {
            break;
        } else {
            let command: Command = serde_json::from_value(value).unwrap();
            match pointer.as_mut().unwrap().send(command).await {
                Ok(status) => serde_json::json!({"status":format!("{status:?}")}),
                Err(error) => serde_json::json!({"error":error.to_string()}),
            }
        };
        println!("POINTER {result}");
        std::io::stdout().flush().unwrap();
    }
    if let Some(pointer) = pointer {
        pointer.stop().await.unwrap();
    }
    recording.stop().await.unwrap();
    println!("POINTER {{\"stopped\":true}}");
    std::io::stdout().flush().unwrap();
}
