//! Launched by scripts/test-greetd-transport.py inside a private user/PID namespace.
use canopa_greeter::{
    config::Session,
    protocol::{Command, Kind, Worker},
};
use canopa_session_ui::secret::Secret;
use std::{path::PathBuf, sync::mpsc, time::Duration};

#[test]
#[ignore = "requires the isolated greetd/PAM runner; never run against a host login socket"]
fn failed_password_retries_then_hands_off_to_real_greetd() {
    assert_eq!(
        std::env::var("CANOPA_GREETD_ACCEPTANCE").as_deref(),
        Ok("1")
    );
    let root = PathBuf::from(std::env::var_os("CANOPA_GREETD_ROOT").unwrap());
    let socket = PathBuf::from(std::env::var_os("GREETD_SOCK").unwrap());
    let uid_map = std::fs::read_to_string("/proc/self/uid_map").unwrap();
    let mapping: Vec<u64> = uid_map
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect();
    assert_eq!(mapping.len(), 3);
    assert_eq!((mapping[0], mapping[2]), (0, 1));
    assert_ne!(
        mapping[1], 0,
        "namespace root must map to an unprivileged user"
    );
    // The runner mounts a private /run; greetd may choose its default socket there.
    assert!(socket.parent() == Some(root.as_path()) || socket.parent() == Some("/run".as_ref()));
    let (sender, receiver) = mpsc::channel();
    let worker = Worker::start(socket, 20, move |event| {
        sender.send(event).unwrap();
    });
    worker
        .send(Command::Begin {
            generation: 1,
            user: "root".into(),
        })
        .unwrap();
    let mut failed = false;
    loop {
        let event = receiver.recv_timeout(Duration::from_secs(25)).unwrap();
        match event.kind {
            Kind::Prompt { echo, .. } => {
                assert!(!echo);
                let mut answer = Secret::default();
                answer.push(if event.generation == 1 {
                    "wrong-password"
                } else {
                    "canopa-fixture-password"
                });
                worker
                    .send(Command::Answer {
                        generation: event.generation,
                        answer,
                    })
                    .unwrap();
            }
            Kind::Failed(_) => {
                assert_eq!(event.generation, 1);
                assert!(!failed);
                failed = true;
                worker
                    .send(Command::Begin {
                        generation: 2,
                        user: "root".into(),
                    })
                    .unwrap();
            }
            Kind::Authenticated => {
                assert!(failed);
                assert_eq!(event.generation, 2);
                worker
                    .send(Command::Commit {
                        generation: 2,
                        session: Session {
                            id: "fixture".into(),
                            label: "Fixture".into(),
                            argv: vec![
                                "/usr/bin/python3".into(),
                                root.join("session.py").to_str().unwrap().into(),
                                "arg with spaces".into(),
                                "'quote\"".into(),
                                format!("$(touch {}/unexpected)", root.display()),
                                String::new(),
                            ],
                            env: vec!["CANOPA_HANDOFF=literal value".into()],
                            comment: None,
                            desktop_file: None,
                        },
                    })
                    .unwrap();
            }
            Kind::Started => {
                assert!(failed);
                std::fs::write(root.join("greeter-finished"), b"ok").unwrap();
                break;
            }
            Kind::Fatal => panic!("real greetd transport failed"),
            Kind::Cancelled(_) => panic!("unexpected cancellation"),
            Kind::Message { .. } => {}
        }
    }
}
