use std::{
    io::{Read, Write},
    os::unix::net::UnixListener,
    path::Path,
    process::{Command, Output, Stdio},
    time::Duration,
};

fn cli(args: &[&str], runtime: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canopa-shell"))
        .args(args)
        .env("XDG_RUNTIME_DIR", runtime)
        .env("XDG_STATE_HOME", runtime.join("state"))
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn menu_cli_reports_usage_errors_without_starting_a_bar() {
    let dir = tempfile::tempdir().unwrap();
    for flag in ["--open-menu", "--toggle-menu"] {
        for args in [
            vec!["--open-menu"],
            vec!["--open-menu", ""],
            vec!["--open-menu="],
            vec!["--open-menu", "-V"],
            vec!["--open-menu", "--toggle-panel"],
            vec!["--open-menu", "apps", "--toggle-panel"],
            vec!["--toggle-panel", "--open-menu", "apps"],
            vec!["--open-menu", "apps", "--dmenu"],
            vec!["--open-menu", "apps", "--check-config"],
            vec!["--open-menu", "apps", "--defaults"],
            vec!["--open-menu", "apps", "--show-source"],
            vec!["--open-menu", "apps", "--prompt", "Pick"],
            vec!["--json", "--open-menu", "apps"],
            vec!["--open-menu", "apps", "extra"],
            vec!["--open-menu", "apps", "--open-menu", "main"],
            vec!["--open-menu", "apps", "--toggle-menu", "main"],
            vec!["--toggle-menu", "apps", "--open-menu", "main"],
        ] {
            let args: Vec<_> = args
                .iter()
                .map(|arg| arg.replace("--open-menu", flag))
                .collect();
            let argv: Vec<_> = args.iter().map(String::as_str).collect();
            let output = cli(&argv, dir.path());
            assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
            assert!(!output.stderr.is_empty());
        }
    }
    assert!(!dir.path().join("canopa-shell.sock").exists());
}

#[test]
fn menu_cli_requires_a_running_bar() {
    let dir = tempfile::tempdir().unwrap();
    for (flag, diagnostic) in [
        ("--open-menu", "open-menu request failed"),
        ("--toggle-menu", "toggle-menu request failed"),
    ] {
        let output = cli(&[flag, "apps"], dir.path());
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
    }
}

#[test]
fn menu_cli_sends_literal_names_and_checks_the_server_response() {
    // Exercise the executable against the wire contract, independently of its encoder.
    for (flag, kind) in [
        ("--open-menu", "open-menu"),
        ("--toggle-menu", "toggle-menu"),
    ] {
        for (name, response, code, diagnostic) in [
            ("apps", r#"{"type":"accepted"}"#, 0, ""),
            ("--tools", r#"{"type":"accepted"}"#, 0, ""),
            ("My tools ☰\n\"quoted\"", r#"{"type":"accepted"}"#, 0, ""),
            (
                "missing",
                r#"{"type":"rejected","reason":"unknown menu"}"#,
                1,
                "unknown menu",
            ),
            (
                "apps",
                r#"{"type":"cancelled"}"#,
                1,
                "unexpected open-menu response",
            ),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let listener = UnixListener::bind(dir.path().join("canopa-shell.sock")).unwrap();
            listener.set_nonblocking(true).unwrap();
            let server = std::thread::spawn(move || {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                let mut socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(std::time::Instant::now() < deadline, "CLI never connected");
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => panic!("accept: {error}"),
                    }
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                socket.read_to_end(&mut request).unwrap();
                let magic = b"NIRIBAR_IPC_JSON_V1\n";
                assert!(request.starts_with(magic));
                let payload = &request[magic.len() + 8..];
                let size =
                    u64::from_be_bytes(request[magic.len()..magic.len() + 8].try_into().unwrap());
                assert_eq!(size as usize, payload.len());
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(payload).unwrap(),
                    serde_json::json!({"type": kind, "name": name})
                );
                socket.write_all(magic).unwrap();
                socket
                    .write_all(&(response.len() as u64).to_be_bytes())
                    .unwrap();
                socket.write_all(response.as_bytes()).unwrap();
            });
            let inline_arg = format!("{flag}={name}");
            let args = if name.starts_with('-') {
                vec![inline_arg.as_str()]
            } else {
                vec![flag, name]
            };
            let output = cli(&args, dir.path());
            server.join().unwrap();
            assert_eq!(output.status.code(), Some(code), "{output:?}");
            assert!(output.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains(&diagnostic.replace("open-menu", kind))
            );
        }
    }
}
