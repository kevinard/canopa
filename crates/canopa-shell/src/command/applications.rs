//! Application lifetime belongs to the desktop session, never to Canopa Shell's workers.
use super::SpawnResult;
use std::io;
use std::path::Path;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

pub async fn launch_application(argv: &[String]) -> io::Result<SpawnResult> {
    let program = argv
        .first()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| io::Error::other("application launch requires a program"))?;
    let executable = executable_path(program).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "application executable is unavailable",
        )
    })?;
    // Resolve against this launcher's cwd/PATH before handing off to niri,
    // whose working directory and PATH can differ.
    let mut argv = argv.to_vec();
    argv[0] = executable
        .to_str()
        .ok_or_else(|| io::Error::other("application executable path is not UTF-8"))?
        .into();
    if let Some(path) = std::env::var_os(niri_ipc::socket::SOCKET_PATH_ENV) {
        // A failed/ambiguous handoff must not be retried through a second launcher.
        launch_through_niri(Path::new(&path), &argv).await?;
    } else {
        // --scope moves the launcher out of canopa-shell.service before it execs the app.
        // Scope mode also preserves the caller's graphical-session environment.
        let mut child = std::process::Command::new("systemd-run")
            .args(["--user", "--scope", "--collect", "--quiet", "--"])
            .args(argv)
            .stdin(std::process::Stdio::null())
            .spawn()?;
        std::thread::spawn(move || match child.wait() {
            Ok(status) if status.success() => {}
            result => {
                tracing::warn!(target: "canopa_shell::launcher", ?result, "application scope exited unsuccessfully; see launcher stderr")
            }
        });
    }
    Ok(SpawnResult::Spawned)
}

fn executable_path(program: &str) -> Option<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let executable = |path: &Path| {
        path.metadata()
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    };
    if program.contains('/') {
        return executable(Path::new(program))
            .then(|| std::path::absolute(program).ok())
            .flatten();
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|path| path.join(program))
        .find(|path| executable(path))
        .and_then(|path| std::path::absolute(path).ok())
}

async fn launch_through_niri(path: &Path, argv: &[String]) -> io::Result<()> {
    niri_action(
        path,
        niri_ipc::Action::Spawn {
            command: argv.to_vec(),
        },
    )
    .await
}
pub async fn focus_window(id: u64) -> io::Result<()> {
    let path = std::env::var_os(niri_ipc::socket::SOCKET_PATH_ENV)
        .ok_or_else(|| io::Error::other("Niri is unavailable"))?;
    niri_action(Path::new(&path), niri_ipc::Action::FocusWindow { id }).await
}
async fn niri_action(path: &Path, action: niri_ipc::Action) -> io::Result<()> {
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut stream = tokio::net::UnixStream::connect(path).await?;
        let request = niri_ipc::Request::Action(action);
        let mut request = serde_json::to_vec(&request)?;
        request.push(b'\n');
        stream.write_all(&request).await?;
        let mut line = String::new();
        tokio::io::BufReader::new(stream)
            .take(65537)
            .read_line(&mut line)
            .await?;
        if line.len() > 65536 {
            return Err(io::Error::other("oversized niri launch response"));
        }
        match serde_json::from_str::<niri_ipc::Reply>(&line)
            .map_err(|_| io::Error::other("invalid niri launch response"))?
        {
            Ok(niri_ipc::Response::Handled) => Ok(()),
            _ => Err(io::Error::other("niri rejected application launch")),
        }
    })
    .await
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::TimedOut,
            "niri application handoff timed out",
        )
    })?
}

/// The compositor accepts a spawn before `gio launch` resolves the entry, and
/// the child's output is discarded, so check what would otherwise fail unseen.
fn check_desktop_launch(
    app: &crate::desktop_apps::DesktopApplication,
    search_path: &[std::path::PathBuf],
) -> io::Result<()> {
    crate::desktop_apps::check_launch(app, search_path).map_err(|problem| {
        let kind = match problem {
            crate::desktop_apps::LaunchProblem::InvalidCommand => io::ErrorKind::InvalidData,
            _ => io::ErrorKind::NotFound,
        };
        io::Error::new(kind, problem)
    })
}

fn desktop_launch_argv(app: &crate::desktop_apps::DesktopApplication) -> io::Result<Vec<String>> {
    // GIO owns Exec field codes and cwd for non-D-Bus desktop entries.
    // Never reconstruct an Exec line or pass it through a shell here.
    // Keep the selected basename: it defines the D-Bus application identity,
    // even when the entry is a symlink to a differently named file.
    let path = std::path::absolute(&app.desktop_file)?;
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "desktop entry is unavailable",
        ));
    }
    let path = path
        .to_str()
        .ok_or_else(|| io::Error::other("desktop entry path is not UTF-8"))?;
    Ok(vec!["gio".into(), "launch".into(), path.into()])
}

#[cfg(not(test))]
pub async fn spawn_desktop_application_fire_and_forget(
    app: &crate::desktop_apps::DesktopApplication,
) -> io::Result<SpawnResult> {
    check_desktop_launch(app, &crate::desktop_apps::process_search_path())?;
    if app.dbus_activatable {
        // Auto-activation is owned by the session bus, outside the bar's cgroup.
        // Await delivery instead of letting a short-lived CLI exit mid-flush.
        tokio::time::timeout(Duration::from_secs(10), async {
            let connection = zbus::Connection::session().await.map_err(|_| {
                io::Error::other("session bus unavailable for application activation")
            })?;
            activate_dbus_application(&connection, app).await
        })
        .await
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                "desktop application activation timed out",
            )
        })??;
        Ok(SpawnResult::Spawned)
    } else {
        launch_application(&desktop_launch_argv(app)?).await
    }
}

#[cfg(not(test))]
pub async fn open_desktop_application(
    app: &crate::desktop_apps::DesktopApplication,
    uris: &[String],
) -> io::Result<SpawnResult> {
    check_desktop_launch(app, &crate::desktop_apps::process_search_path())?;
    if app.dbus_activatable {
        tokio::time::timeout(Duration::from_secs(10), async {
            let connection = zbus::Connection::session()
                .await
                .map_err(io::Error::other)?;
            dbus_application_request(&connection, app, Some(uris)).await
        })
        .await
        .map_err(|_| io::Error::other("Application Open timed out"))??;
        Ok(SpawnResult::Spawned)
    } else {
        let mut argv = desktop_launch_argv(app)?;
        argv.extend_from_slice(uris);
        launch_application(&argv).await
    }
}

async fn activate_dbus_application(
    connection: &zbus::Connection,
    app: &crate::desktop_apps::DesktopApplication,
) -> io::Result<()> {
    dbus_application_request(connection, app, None).await
}
async fn dbus_application_request(
    connection: &zbus::Connection,
    app: &crate::desktop_apps::DesktopApplication,
    uris: Option<&[String]>,
) -> io::Result<()> {
    // The specification defines this from the selected filename, not its
    // symlink target or the discovery ID's directory prefix.
    let name = app
        .desktop_file
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".desktop"))
        .ok_or_else(|| io::Error::other("invalid D-Bus application filename"))?;
    let name = zbus::names::WellKnownName::try_from(name)
        .map_err(|_| io::Error::other("invalid D-Bus application name"))?;
    let path = format!("/{}", name.as_str().replace('.', "/").replace('-', "_"));
    let result = async {
        let proxy = zbus::Proxy::new(connection, name, path, "org.freedesktop.Application").await?;
        // Default D-Bus call flags allow the bus to start a missing owner.
        let platform_data = std::collections::HashMap::<String, zbus::zvariant::Value<'_>>::new();
        match uris {
            Some(uris) => proxy.call::<_, _, ()>("Open", &(uris, platform_data)).await,
            None => proxy.call::<_, _, ()>("Activate", &(platform_data,)).await,
        }
    }
    .await;
    result.map_err(|error| match error {
        zbus::Error::MethodError(name, _, _) => {
            io::Error::other(format!("desktop activation rejected ({name})"))
        }
        _ => io::Error::other("desktop activation transport failed"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, Write};
    use std::os::unix::fs::PermissionsExt;

    #[tokio::test]
    async fn trickling_compositor_response_cannot_extend_handoff_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("niri.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let server = tokio::spawn(async move {
            let (mut client, _) = listener.accept().await.unwrap();
            loop {
                if client.write_all(b" ").await.is_err() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });
        let error = tokio::time::timeout(
            Duration::from_secs(3),
            launch_through_niri(&socket, &["app".into()]),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        server.abort();
        let _ = server.await;
    }
    #[tokio::test]
    async fn handed_off_application_outlives_the_launch_client_and_command_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("niri.sock");
        let marker = dir.path().join("survived");
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            let (mut client, _) = listener.accept().unwrap();
            let mut line = String::new();
            std::io::BufReader::new(client.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            let niri_ipc::Request::Action(niri_ipc::Action::Spawn { command }) =
                serde_json::from_str(&line).unwrap()
            else {
                panic!("spawn request");
            };
            let mut app = std::process::Command::new(&command[0])
                .args(&command[1..])
                .spawn()
                .unwrap();
            client.write_all(b"{\"Ok\":\"Handled\"}\n").unwrap();
            drop(client);
            assert!(app.wait().unwrap().success());
        });
        let command = vec![
            "sh".into(),
            "-c".into(),
            "sleep 3.2; printf survived > \"$1\"".into(),
            "fixture".into(),
            marker.to_str().unwrap().into(),
        ];
        // The launcher exits before the compositor-owned process finishes.
        tokio::spawn(async move { launch_through_niri(&socket, &command).await.unwrap() })
            .await
            .unwrap();
        server.join().unwrap();
        assert_eq!(std::fs::read_to_string(marker).unwrap(), "survived");
    }

    struct BusApplication {
        activations: tokio::sync::mpsc::UnboundedSender<()>,
        opened: tokio::sync::mpsc::UnboundedSender<Vec<String>>,
        reject: bool,
    }
    #[zbus::interface(name = "org.freedesktop.Application")]
    impl BusApplication {
        fn open(
            &self,
            uris: Vec<String>,
            _platform_data: std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        ) -> zbus::fdo::Result<()> {
            if self.reject {
                return Err(zbus::fdo::Error::AccessDenied(
                    "private fixture diagnostic".into(),
                ));
            }
            self.opened.send(uris).unwrap();
            Ok(())
        }

        fn activate(
            &self,
            _platform_data: std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        ) -> zbus::fdo::Result<()> {
            if self.reject {
                return Err(zbus::fdo::Error::AccessDenied(
                    "private fixture diagnostic".into(),
                ));
            }
            self.activations.send(()).unwrap();
            Ok(())
        }
    }

    #[tokio::test]
    async fn dbus_only_entry_preserves_filename_identity_and_acknowledges_activation() {
        let bus = crate::test_bus::TestBus::new();
        let service = bus.connect().await;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let (opened, mut opened_rx) = tokio::sync::mpsc::unbounded_channel();
        service
            .object_server()
            .at(
                "/org/example/Fixture_With_Dashes",
                BusApplication {
                    activations: tx,
                    opened,
                    reject: false,
                },
            )
            .await
            .unwrap();
        service
            .request_name("org.example.Fixture-With-Dashes")
            .await
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let desktop_file = dir.path().join("org.example.Fixture-With-Dashes.desktop");
        let backing_file = dir.path().join("backing.desktop");
        std::fs::write(
            &backing_file,
            "[Desktop Entry]\nType=Application\nName=Bus App\nDBusActivatable=true\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(&backing_file, &desktop_file).unwrap();
        let app = crate::desktop_apps::applications_from_entries(
            vec![crate::desktop_apps::RawDesktopEntry {
                id: "nested-org.example.Fixture-With-Dashes.desktop".into(),
                body: std::fs::read_to_string(&desktop_file).unwrap(),
                path: desktop_file,
            }],
            &crate::desktop_apps::DesktopEnvironment::unknown(),
        )
        .pop()
        .unwrap();
        assert!(app.dbus_activatable);
        let client = bus.connect().await;
        for _ in 0..20 {
            activate_dbus_application(&client, &app).await.unwrap();
            rx.try_recv()
                .expect("an acknowledged activation has been delivered");
            assert!(rx.try_recv().is_err(), "exactly one activation per request");
        }
        let uris = vec!["file:///tmp/two%20words%FF.txt".to_owned()];
        dbus_application_request(&client, &app, Some(&uris))
            .await
            .unwrap();
        assert_eq!(opened_rx.try_recv().unwrap(), uris);
        assert!(rx.try_recv().is_err());
        let application = service
            .object_server()
            .interface::<_, BusApplication>("/org/example/Fixture_With_Dashes")
            .await
            .unwrap();
        application.get_mut().await.reject = true;
        assert!(
            dbus_application_request(&client, &app, Some(&uris))
                .await
                .is_err()
        );
        assert!(opened_rx.try_recv().is_err());
        let error = activate_dbus_application(&client, &app).await.unwrap_err();
        assert!(error.to_string().contains("AccessDenied"));
        assert!(!error.to_string().contains("private fixture diagnostic"));
        service.close().await.unwrap();
        assert!(
            activate_dbus_application(&client, &app)
                .await
                .unwrap_err()
                .to_string()
                .contains("ServiceUnknown")
        );
    }

    #[tokio::test]
    async fn compositor_launch_preserves_arguments_and_returns_rejection() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("niri.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            let (mut client, _) = listener.accept().unwrap();
            let mut line = String::new();
            std::io::BufReader::new(client.try_clone().unwrap())
                .read_line(&mut line)
                .unwrap();
            let request: niri_ipc::Request = serde_json::from_str(&line).unwrap();
            assert!(
                matches!(request, niri_ipc::Request::Action(niri_ipc::Action::Spawn { command }) if command == ["app", "two words", "$(literal)"])
            );
            client.write_all(b"{\"Err\":\"launch denied\"}\n").unwrap();
        });
        let error = launch_through_niri(
            &socket,
            &["app".into(), "two words".into(), "$(literal)".into()],
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("niri rejected"));
        server.join().unwrap();
    }

    #[tokio::test]
    async fn focus_window_uses_exact_id_and_requires_compositor_acknowledgement() {
        for reply in [
            b"{\"Ok\":\"Handled\"}\n".as_slice(),
            b"{\"Err\":\"window gone\"}\n".as_slice(),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let socket = dir.path().join("niri.sock");
            let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
            let reply = reply.to_vec();
            let accepted = reply.starts_with(b"{\"Ok\"");
            let server = std::thread::spawn(move || {
                let (mut client, _) = listener.accept().unwrap();
                let mut line = String::new();
                std::io::BufReader::new(client.try_clone().unwrap())
                    .read_line(&mut line)
                    .unwrap();
                assert!(matches!(
                    serde_json::from_str(&line).unwrap(),
                    niri_ipc::Request::Action(niri_ipc::Action::FocusWindow { id: 42 })
                ));
                client.write_all(&reply).unwrap();
            });
            assert_eq!(
                niri_action(&socket, niri_ipc::Action::FocusWindow { id: 42 })
                    .await
                    .is_ok(),
                accepted
            );
            server.join().unwrap();
        }
    }

    fn executable(path: &Path, body: &str) {
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn fixture_app(desktop_file: std::path::PathBuf) -> crate::desktop_apps::DesktopApplication {
        crate::desktop_apps::DesktopApplication {
            id: "org.example.Fixture.desktop".into(),
            name: "Fixture".into(),
            generic_name: None,
            comment: None,
            keywords: vec![],
            icon: None,
            desktop_file,
            dbus_activatable: false,
        }
    }

    #[test]
    fn launcher_rejects_missing_programs_and_terminals_before_handoff() {
        let dir = tempfile::tempdir().unwrap();
        let bin = [dir.path().to_path_buf()];
        let desktop_file = dir.path().join("org.example.Fixture.desktop");
        let app = fixture_app(desktop_file.clone());

        std::fs::write(
            &desktop_file,
            "[Desktop Entry]\nType=Application\nName=Fixture\nExec=does-not-exist %U\n",
        )
        .unwrap();
        let error = check_desktop_launch(&app, &bin).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(error.to_string(), "“does-not-exist” is not installed");

        executable(&dir.path().join("htop"), "#!/bin/sh\n");
        std::fs::write(
            &desktop_file,
            "[Desktop Entry]\nType=Application\nName=Fixture\nExec=htop\nTerminal=true\n",
        )
        .unwrap();
        let error = check_desktop_launch(&app, &bin).unwrap_err();
        assert!(error.to_string().contains("xdg-terminal-exec"), "{error}");

        executable(&dir.path().join("xdg-terminal-exec"), "#!/bin/sh\n");
        check_desktop_launch(&app, &bin).unwrap();

        std::fs::remove_file(&desktop_file).unwrap();
        assert_eq!(
            check_desktop_launch(&app, &bin).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn gio_launch_runs_terminal_entries_through_xdg_terminal_exec() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("output");
        executable(
            &dir.path().join("xdg-terminal-exec"),
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n",
                output.display()
            ),
        );
        executable(&dir.path().join("tui-fixture"), "#!/bin/sh\n");
        let desktop_file = dir.path().join("org.example.Fixture.desktop");
        std::fs::write(
            &desktop_file,
            "[Desktop Entry]\nType=Application\nName=Fixture\nExec=tui-fixture --flag\nTerminal=true\n",
        )
        .unwrap();
        let app = fixture_app(desktop_file);
        let mut path = std::ffi::OsString::from(dir.path());
        path.push(":");
        path.push(std::env::var_os("PATH").unwrap_or_default());
        let search_path: Vec<_> = std::env::split_paths(&path).collect();
        check_desktop_launch(&app, &search_path).unwrap();
        let argv = desktop_launch_argv(&app).unwrap();
        assert!(
            std::process::Command::new(&argv[0])
                .args(&argv[1..])
                .env("PATH", &path)
                .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent")
                .status()
                .unwrap()
                .success()
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while !std::fs::read_to_string(&output).is_ok_and(|text| text.ends_with("--flag\n"))
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        let arguments = std::fs::read_to_string(&output).unwrap();
        assert!(
            arguments.lines().any(|line| line == "tui-fixture") && arguments.ends_with("--flag\n"),
            "{arguments}"
        );
    }

    #[test]
    fn gio_launch_preserves_desktop_metadata_and_working_directory() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("capture");
        let output = dir.path().join("output");
        std::fs::write(
            &script,
            "#!/bin/sh\nout=$1\nshift\nprintf '%s\\n' \"$PWD\" \"$@\" > \"$out\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        let desktop_file = dir.path().join("org.example.Fixture.desktop");
        std::fs::write(&desktop_file, format!("[Desktop Entry]\nType=Application\nName=Fixture Name\nIcon=fixture-icon\nPath={}\nExec={} {} %c %i %k %U\n", dir.path().display(), script.display(), output.display())).unwrap();
        let app = crate::desktop_apps::DesktopApplication {
            id: "org.example.Fixture.desktop".into(),
            name: "Fixture Name".into(),
            generic_name: None,
            comment: None,
            keywords: vec![],
            icon: Some("fixture-icon".into()),
            desktop_file: desktop_file.clone(),
            dbus_activatable: false,
        };
        let argv = desktop_launch_argv(&app).unwrap();
        assert!(
            std::process::Command::new(&argv[0])
                .args(&argv[1..])
                .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent")
                .status()
                .unwrap()
                .success()
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let expected = format!(
            "{}\nFixture Name\n--icon\nfixture-icon\n{}\n",
            dir.path().display(),
            desktop_file.display()
        );
        while std::fs::read_to_string(&output).ok().as_deref() != Some(expected.as_str())
            && std::time::Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(std::fs::read_to_string(output).unwrap(), expected);
    }
}
