use canopa_config::Request;
use canopa_greeter::{
    config::Config,
    controller::{Controller, Phase},
    sessions::Discovery,
};
use std::{fs, os::unix::fs::PermissionsExt, path::Path};

fn entry(dir: &Path, id: &str, extra: &str) {
    fs::write(
        dir.join(format!("{id}.desktop")),
        format!("[Desktop Entry]\nType=Application\nName={id}\nExec=/usr/bin/true\n{extra}"),
    )
    .unwrap();
}
fn discovery(dirs: Vec<std::path::PathBuf>) -> Discovery {
    Discovery {
        wayland_dirs: dirs,
        x11_dirs: Vec::new(),
        ..Discovery::default()
    }
}

#[test]
fn directory_precedence_visibility_tryexec_and_invalid_files_do_not_execute_commands() {
    let root = tempfile::tempdir().unwrap();
    let high = root.path().join("high");
    let low = root.path().join("low");
    fs::create_dir(&high).unwrap();
    fs::create_dir(&low).unwrap();
    entry(&low, "masked", "");
    entry(&high, "masked", "Hidden=true\n");
    entry(
        &high,
        "good",
        "DesktopNames=Example;Other;\nTryExec=/usr/bin/true\n",
    );
    entry(&high, "hidden", "NoDisplay=true\n");
    entry(&high, "terminal", "Terminal=true\n");
    entry(&high, "dbus", "DBusActivatable=true\n");
    entry(&high, "gone", "TryExec=/no/such/session\n");
    entry(&high, "malformed", "Hidden=maybe\n");
    fs::write(
        high.join("duplicate.desktop"),
        "[Desktop Entry]\nType=Application\nName=A\nName=B\nExec=/usr/bin/true\n",
    )
    .unwrap();
    let marker = root.path().join("executed");
    let probe = root.path().join("probe");
    fs::write(&probe, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
    fs::set_permissions(&probe, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(
        high.join("probe.desktop"),
        format!(
            "[Desktop Entry]\nType=Application\nName=Probe\nTryExec={}\nExec={}\n",
            probe.display(),
            probe.display()
        ),
    )
    .unwrap();
    let found = discovery(vec![high.clone(), low]).scan(false);
    assert_eq!(
        found.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["good", "probe"]
    );
    assert_eq!(
        found[0].env,
        [
            "XDG_SESSION_TYPE=wayland",
            "XDG_SESSION_DESKTOP=good",
            "XDG_CURRENT_DESKTOP=Example:Other"
        ]
    );
    assert!(!marker.exists(), "discovery executed a session or TryExec");
    assert!(
        discovery(vec![high]).scan(true).is_empty(),
        "untrusted sources must not enter normal system discovery"
    );
}

#[test]
fn desktop_exec_roundtrips_literal_arguments_field_codes_and_working_directory() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("entries with spaces");
    fs::create_dir(&dir).unwrap();
    let script = dir.join("capture.py");
    fs::write(&script,"import json,os,sys\nprint(json.dumps([sys.argv[1:],os.getcwd(),os.environ['XDG_SESSION_TYPE']]))\n").unwrap();
    let marker = dir.join("must-not-run");
    let literal = format!("$(touch {})", marker.display());
    let args = [
        "/usr/bin/python3",
        script.to_str().unwrap(),
        "arg with spaces",
        "'quote\"",
        literal.as_str(),
        "",
    ];
    let command = args
        .iter()
        .map(|a| {
            format!(
                "\"{}\"",
                a.replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('$', "\\$")
                    .replace('`', "\\`")
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    let command = command.replace('\\', "\\\\");
    let path = dir.join("literal.desktop");
    fs::write(&path,format!("[Desktop Entry]\nType=Application\nName=My Desktop\nExec={command} %% %c %k %i %F\nIcon=test-icon\nPath={}\n",dir.display())).unwrap();
    let sessions = discovery(vec![dir.clone()]).scan(false);
    assert_eq!(sessions.len(), 1);
    let s = &sessions[0];
    let mut process = std::process::Command::new("sh");
    process.args(["-c", &format!("exec {}", s.shell_command().unwrap())]);
    for pair in &s.env {
        let (key, value) = pair.split_once('=').unwrap();
        process.env(key, value);
    }
    let output = process.output().unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        result,
        serde_json::json!([
            [
                "arg with spaces",
                "'quote\"",
                literal,
                "",
                "%",
                "My Desktop",
                path.to_str().unwrap(),
                "--icon",
                "test-icon"
            ],
            dir.to_str().unwrap(),
            "wayland"
        ])
    );
    assert!(!marker.exists());
}

#[test]
fn manual_override_curated_mode_and_empty_catalog_are_host_independent() {
    let root = tempfile::tempdir().unwrap();
    entry(root.path(), "override", "");
    entry(root.path(), "zulu", "");
    let file = root.path().join("config.kdl");
    let source = format!(
        "schema-version 1\ngreeter {{ session-dirs \"{}\"; session \"override\" {{ label \"Manual override\"; command {{ argv \"/usr/bin/true\" \"custom\"; }}; }}; }}\n",
        root.path().display()
    );
    fs::write(&file, &source).unwrap();
    let (mut config, _) = Config::load(&Request::common(&file)).unwrap();
    assert_eq!(config.sessions.len(), 1, "schema validation must not scan");
    config.resolve_sessions(false);
    assert_eq!(config.sessions.len(), 2);
    assert_eq!(config.sessions[0].argv, ["/usr/bin/true", "custom"]);
    assert!(config.sessions[0].desktop_file.is_none());
    fs::write(
        &file,
        source.replace("greeter {", "greeter { discover-sessions #false;"),
    )
    .unwrap();
    let (mut curated, _) = Config::load(&Request::common(&file)).unwrap();
    curated.resolve_sessions(false);
    assert_eq!(curated.sessions.len(), 1);
    fs::write(
        &file,
        "schema-version 1\ngreeter { discover-sessions #false; }\n",
    )
    .unwrap();
    let (mut empty, _) = Config::load(&Request::common(&file)).unwrap();
    empty.resolve_sessions(false);
    let mut controller = Controller::new(&empty).unwrap();
    controller.input("alice");
    assert!(controller.submit().is_none());
    assert!(controller.session_next(&empty).is_none());
    assert!(controller.phase == Phase::User);
    assert!(controller.ui.message.contains("No desktop sessions"));
    assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"))
        .arg("--canopa-config")
        .arg(&file)
        .arg("list-sessions")
        .assert()
        .success()
        .stdout("[]\n");
    assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"))
        .arg("--canopa-config")
        .arg(&file)
        .arg("preview")
        .arg(root.path().join("empty.png"))
        .assert()
        .success();
}

#[test]
fn x11_entries_require_a_wrapper_and_have_separate_ids_and_session_environment() {
    let root = tempfile::tempdir().unwrap();
    entry(root.path(), "desktop", "");
    let mut policy = discovery(Vec::new());
    policy.x11_dirs = vec![root.path().to_owned()];
    assert!(policy.scan(false).is_empty());
    policy.x11_wrapper = Some(vec!["/usr/bin/true".into(), "--fixture-wrapper".into()]);
    let sessions = policy.scan(false);
    assert_eq!(sessions.len(), 1);
    let s = &sessions[0];
    assert_eq!(s.id, "x11:desktop");
    assert_eq!(s.label, "desktop (X11)");
    assert_eq!(
        s.argv,
        ["/usr/bin/true", "--fixture-wrapper", "/usr/bin/true"]
    );
    assert_eq!(s.env[0], "XDG_SESSION_TYPE=x11");
    policy.x11_wrapper = Some(vec!["/no/such/wrapper".into()]);
    assert!(policy.scan(false).is_empty());
}

#[test]
fn oversized_nonregular_and_invalid_utf8_entries_are_skipped_without_blocking() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("huge.desktop"), "x".repeat(65537)).unwrap();
    fs::write(root.path().join("utf8.desktop"), [255, 255, 0]).unwrap();
    let fifo = root.path().join("fifo.desktop");
    let path = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    assert!(
        discovery(vec![root.path().to_owned()])
            .scan(false)
            .is_empty()
    );
}

#[test]
fn user_xdg_directories_are_not_part_of_default_login_session_discovery() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("wayland-sessions");
    fs::create_dir(&dir).unwrap();
    entry(&dir, "canopa-untrusted-user-only", "");
    let output = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"))
        .args(["--defaults", "list-sessions"])
        .env("XDG_DATA_HOME", root.path())
        .env("XDG_DATA_DIRS", root.path())
        .env("HOME", root.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let sessions: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(
        !sessions
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == "canopa-untrusted-user-only")
    );
}

#[test]
fn absolute_path_search_components_support_nonstandard_system_prefixes_without_execution() {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    let sessions = root.path().join("sessions");
    fs::create_dir(&bin).unwrap();
    fs::create_dir(&sessions).unwrap();
    let program = bin.join("canopa-fixture-session");
    let marker = root.path().join("executed");
    fs::write(
        &program,
        format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    )
    .unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(sessions.join("custom.desktop"),"[Desktop Entry]\nType=Application\nName=Custom\nExec=canopa-fixture-session\nTryExec=canopa-fixture-session\n").unwrap();
    let config = root.path().join("greeter.kdl");
    fs::write(
        &config,
        format!(
            "schema-version 1\ngreeter {{ session-dirs \"{}\"; }}\n",
            sessions.display()
        ),
    )
    .unwrap();
    let data = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"))
        .arg("--canopa-config")
        .arg(&config)
        .arg("list-sessions")
        .env("PATH", format!("relative:{}", bin.display()))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let found: serde_json::Value = serde_json::from_slice(&data).unwrap();
    assert_eq!(found[0]["argv"][0], program.to_str().unwrap());
    assert!(!marker.exists());
}

#[test]
fn discovery_schema_rejects_ambiguous_settings_and_accepts_explicit_empty_roots() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("greeter.kdl");
    for settings in [
        "discover-sessions \"true\"",
        "discover-sessions #true #false",
        "session-dirs \"relative\"",
        "session-dirs { path \"/usr/share/wayland-sessions\"; }",
        "x11-wrapper { argv; }",
        "x11-wrapper { command \"startx\"; }",
        "discover-sessions #true; discover-sessions #false",
    ] {
        fs::write(
            &config,
            format!("schema-version 1\ngreeter {{ {settings}; }}\n"),
        )
        .unwrap();
        assert!(
            Config::load(&Request::common(&config)).is_err(),
            "{settings}"
        );
    }
    fs::write(
        &config,
        "schema-version 1\ngreeter { session-dirs; xsession-dirs; }\n",
    )
    .unwrap();
    let (mut cfg, _) = Config::load(&Request::common(&config)).unwrap();
    cfg.resolve_sessions(false);
    assert!(cfg.sessions.is_empty());
}
