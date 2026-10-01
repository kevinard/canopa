mod support;

use canopa_notify::{Daemon, DaemonOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::Child;
use std::time::Duration;
use support::notifications::notification_with_body;

struct Process(Child);

impl Drop for Process {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}

fn request(socket: &std::path::Path, line: &str) -> String {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(line.as_bytes()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn accept_cli(listener: &UnixListener) -> UnixStream {
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(std::time::Instant::now() < deadline, "CLI did not connect");
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("{error}"),
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
}

fn cli_with_reply(arguments: &[&str], reply: Vec<u8>) -> std::process::Output {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("peer.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let peer = std::thread::spawn(move || {
        let mut reader = BufReader::new(accept_cli(&listener));
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert!(line.ends_with('\n'));
        let _ = reader.get_mut().write_all(&reply);
    });
    let output = assert_cmd::Command::cargo_bin("canopa-notify")
        .unwrap()
        .args(["ctl", "--socket", support::path_arg(&socket)])
        .args(arguments)
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();
    peer.join().unwrap();
    output
}

#[test]
fn cli_accepts_exact_reply_limit_and_rejects_overflow_without_partial_output() {
    for length in [8_388_608, 8_388_609] {
        let reply = format!("[]{}\n", " ".repeat(length - 3));
        let output = cli_with_reply(&["active", "--json"], reply.as_bytes().to_vec());
        if length == 8_388_608 {
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, reply.as_bytes());
        } else {
            assert!(!output.status.success(), "oversized reply was accepted");
            assert!(output.stdout.is_empty(), "partial reply was published");
            assert!(String::from_utf8_lossy(&output.stderr).contains("8 MiB"));
        }
    }
}

#[test]
fn cli_rejects_incomplete_malformed_and_mismatched_replies_without_echoing_payloads() {
    let cases: &[(&[&str], &[u8])] = &[
        (&["status"], b"Canopa Notify daemon available"),
        (&["status"], b"private unexpected success\n"),
        (&["active", "--json"], b"private invalid JSON\n"),
        (
            &["active", "--json"],
            b"[\"private malformed notification\"]\n",
        ),
        (&["active"], b"private active count\n"),
        (&["active"], b"-1 active notifications\n"),
        (&["dnd"], b"{\"reply\":\"private-unknown-variant\"}\n"),
        (&["dnd"], b"{\"reply\":\"command-applied\"}\n"),
        (
            &["clear"],
            b"{\"reply\":\"do-not-disturb\",\"value\":true}\n",
        ),
        (&["status", "--json"], b"{\"reply\":\"command-applied\"}\n"),
        (
            &["reload"],
            b"{\"reply\":\"clear-partially-applied\",\"value\":{\"dismissed\":3}}\n",
        ),
        (&["status"], b"error private daemon response\n"),
        (&["status"], &[0xff, b'\n']),
        (&["status"], b""),
    ];
    for (arguments, reply) in cases {
        let output = cli_with_reply(arguments, reply.to_vec());
        assert!(
            !output.status.success(),
            "invalid reply was accepted for {arguments:?}"
        );
        assert!(
            output.stdout.is_empty(),
            "invalid response reached stdout for {arguments:?}"
        );
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("private"),
            "daemon payload leaked through diagnostics"
        );
    }
}

#[test]
fn cli_preserves_success_formats_and_committed_partial_clear_counts() {
    struct Case {
        arguments: &'static [&'static str],
        reply: &'static [u8],
        stdout: &'static [u8],
        success: bool,
    }
    let cases = [
        Case {
            arguments: &["status"],
            reply: b"Canopa Notify daemon available\n",
            stdout: b"Canopa Notify daemon available\n",
            success: true,
        },
        Case {
            arguments: &["active"],
            reply: b"12 active notifications\n",
            stdout: b"12 active notifications\n",
            success: true,
        },
        Case {
            arguments: &["active", "--json"],
            reply: b"[]\n",
            stdout: b"[]\n",
            success: true,
        },
        Case {
            arguments: &["dnd"],
            reply: b"{\"reply\":\"do-not-disturb\",\"value\":false}\n",
            stdout: b"off\n",
            success: true,
        },
        Case {
            arguments: &["dnd", "on"],
            reply: b"{\"reply\":\"do-not-disturb\",\"value\":true}\n",
            stdout: b"on\n",
            success: true,
        },
        Case {
            arguments: &["clear"],
            reply: b"{\"reply\":\"command-applied\"}\n",
            stdout: b"ok\n",
            success: true,
        },
        Case {
            arguments: &["reload"],
            reply: b"{\"reply\":\"command-applied\"}\n",
            stdout: b"ok\n",
            success: true,
        },
        Case {
            arguments: &["delete-history"],
            reply: b"{\"reply\":\"command-applied\"}\n",
            stdout: b"ok\n",
            success: true,
        },
        Case {
            arguments: &["clear"],
            reply: b"{\"reply\":\"clear-partially-applied\",\"value\":{\"dismissed\":3}}\n",
            stdout: b"",
            success: false,
        },
    ];
    for case in cases {
        let output = cli_with_reply(case.arguments, case.reply.to_vec());
        assert_eq!(output.status.success(), case.success);
        assert_eq!(output.stdout, case.stdout);
        if !case.success {
            assert!(String::from_utf8_lossy(&output.stderr).contains("3 committed dismissals"));
        }
    }
    let reply = canopa_notify::ControlReply::Status(canopa_notify::ControlStatus {
        link_opening: Default::default(),
        available: true,
        do_not_disturb: false,
        configuration: Default::default(),
        presentation_error: Some("é backend unavailable".into()),
        sender_publication_error: None,
        timeout_records: 0,
        timeout_retrying: 0,
        timeout_failed: 0,
    });
    let output = cli_with_reply(
        &["status", "--json"],
        (serde_json::to_string(&reply).unwrap() + "\n").into_bytes(),
    );
    assert!(output.status.success());
    let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["presentation_error"], "é backend unavailable");
    assert_eq!(status["configuration"]["generation"], 0);
    assert!(output.stderr.is_empty());
}

#[test]
fn oversized_cli_request_is_rejected_before_connecting_or_sending() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("peer.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let oversized_path = format!("/{}", "x".repeat(70_000));
    // The peer never replies; local admission must not need a connection or timeout.
    let output = assert_cmd::Command::cargo_bin("canopa-notify")
        .unwrap()
        .args([
            "ctl",
            "--socket",
            support::path_arg(&socket),
            "reload",
            &oversized_path,
        ])
        .timeout(Duration::from_secs(2))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("64 KiB"),
        "request was not rejected locally"
    );
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
        "oversized request connected to the daemon"
    );
}

#[test]
fn cli_request_limit_counts_encoded_bytes_and_accepts_the_exact_boundary() {
    let command = |path: &str| {
        format!(
            "control {}\n",
            serde_json::to_string(&canopa_notify::ControlCommand::ReloadConfiguration {
                path: Some(path.into())
            })
            .unwrap()
        )
    };
    let path = format!("/{}", "x".repeat(65_536 - command("/").len()));
    assert_eq!(command(&path).len(), 65_536);
    let output = cli_with_reply(
        &["reload", &path],
        b"{\"reply\":\"command-applied\"}\n".to_vec(),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"ok\n");

    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("peer.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    // JSON escaping, rather than the path's raw size, crosses the admission limit.
    let escaped_path = format!("/{}", "\\".repeat(35_000));
    assert!(escaped_path.len() < 65_536 && command(&escaped_path).len() > 65_536);
    let output = assert_cmd::Command::cargo_bin("canopa-notify")
        .unwrap()
        .args([
            "ctl",
            "--socket",
            support::path_arg(&socket),
            "reload",
            &escaped_path,
        ])
        .timeout(Duration::from_secs(2))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("64 KiB"));
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
}

#[test]
fn cli_deadline_covers_slow_drip_replies_without_retrying_the_command() {
    let temp = tempfile::tempdir().unwrap();
    let socket = temp.path().join("peer.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let peer = std::thread::spawn(move || {
        let mut reader = BufReader::new(accept_cli(&listener));
        let mut command = String::new();
        reader.read_line(&mut command).unwrap();
        assert_eq!(
            command,
            "control {\"command\":\"clear-active-notifications\"}\n"
        );
        reader
            .get_mut()
            .write_all(b"{\"reply\":\"command-applied\"}")
            .unwrap();
        // Keep each read well within the old 30-second per-read allowance,
        // but never complete the reply. The entire exchange must time out.
        for _ in 0..400 {
            if reader.get_mut().write_all(b" ").is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
            "command was retried"
        );
    });
    let started = std::time::Instant::now();
    let output = assert_cmd::Command::cargo_bin("canopa-notify")
        .unwrap()
        .args(["ctl", "--socket", support::path_arg(&socket), "clear"])
        .timeout(Duration::from_secs(36))
        .output()
        .unwrap();
    let elapsed = started.elapsed();
    peer.join().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("30-second"),
        "missing total deadline diagnostic: {error}"
    );
    assert!(error.contains("outcome may be unknown"));
    assert!(
        elapsed >= Duration::from_secs(29) && elapsed < Duration::from_secs(35),
        "exchange took {elapsed:?}"
    );
}

#[tokio::test]
async fn oversized_control_projection_returns_only_an_error_and_preserves_history() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("history.db");
    let socket = temp.path().join("control.sock");
    let seed = Daemon::open(DaemonOptions::headless(database.clone()))
        .await
        .unwrap();
    for _ in 0..80 {
        seed.receive_notification(notification_with_body(
            "Mail",
            "private summary",
            "x".repeat(60_000),
        ))
        .await
        .unwrap();
    }
    drop(seed);
    let mut daemon = Process(support::spawn_canopa_notify([
        "daemon",
        "--headless",
        "--socket",
        support::path_arg(&socket),
        "--database",
        support::path_arg(&database),
    ]));
    support::wait_for_status_with_child(&socket, &mut daemon.0);
    let response = request(
        &socket,
        "control {\"command\":\"notification-center-projection\"}\n",
    );
    assert!(
        response.len() <= 8 * 1024 * 1024,
        "oversized reply: {} bytes",
        response.len()
    );
    assert_eq!(response, "error control reply exceeds the 8 MiB limit\n");
    assert_eq!(
        request(&socket, "status\n"),
        "Canopa Notify daemon available\n"
    );
    drop(daemon);
    let retained = Daemon::open(DaemonOptions::headless(database))
        .await
        .unwrap();
    let history = retained.snapshot().await.unwrap().history;
    assert_eq!(history.len(), 80);
    assert!(
        history
            .iter()
            .all(|thread| thread.accepted_notification.body.len() == 60_000)
    );
}
