use canopa_greeter::compositor::{self, Compositor, Renderer};
use canopa_theme::Rgb;
use std::{ffi::OsString, os::unix::fs::PermissionsExt, path::Path};

#[test]
fn compositor_argv_preserves_configuration_and_never_uses_a_shell() {
    for compositor in [Compositor::Cage, Compositor::Weston] {
        let args = [
            OsString::from("--canopa-config"),
            OsString::from("/etc/a b/greeter.kdl"),
        ];
        let command = compositor::command(
            compositor,
            Some(Path::new("/opt/with spaces/compositor")),
            Renderer::Software,
            &["--test-option=literal $(touch marker)".into()],
            Path::new("/opt/with spaces/canopa-greeter"),
            &args,
        );
        assert_eq!(command.get_program(), "/opt/with spaces/compositor");
        let argv: Vec<_> = command.get_args().collect();
        let separator = argv.iter().position(|a| *a == "--").unwrap();
        assert_eq!(
            &argv[separator + 1..],
            [
                Path::new("/opt/with spaces/canopa-greeter").as_os_str(),
                args[0].as_os_str(),
                args[1].as_os_str()
            ]
        );
        assert_eq!(argv[separator - 1], "--test-option=literal $(touch marker)");
        match compositor {
            Compositor::Cage => assert!(
                command
                    .get_envs()
                    .any(|(k, v)| k == "WLR_RENDERER" && v == Some("pixman".as_ref()))
            ),
            Compositor::Weston => assert!(argv.contains(&"--renderer=pixman".as_ref())),
            Compositor::Niri => unreachable!(),
        }
    }
}

#[test]
fn selection_is_inspectable_without_greetd_wayland_or_executing_the_compositor() {
    for compositor in ["cage", "weston"] {
        let result = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"))
            .env("CANOPA_LANGUAGE", "en")
            .args([
                "--defaults",
                "session",
                "--compositor",
                compositor,
                "--compositor-bin",
                "/does/not/exist",
                "--print-command",
            ])
            .env_remove("GREETD_SOCK")
            .env_remove("WAYLAND_DISPLAY")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let result: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(result["program"], "/does/not/exist");
        assert_eq!(
            result["argv"].as_array().unwrap().last().unwrap(),
            "--defaults"
        );
    }
    assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"))
        .env("CANOPA_LANGUAGE", "en")
        .args(["session", "--compositor", "invalid", "--print-command"])
        .assert()
        .failure();
}

#[test]
fn niri_is_the_default_and_prints_the_supervised_plan() {
    let output = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"))
        .env("CANOPA_LANGUAGE", "en")
        .env("XDG_RUNTIME_DIR", "/run/user/4242")
        .args([
            "--defaults",
            "session",
            "--compositor-bin",
            "/does/not/exist",
            "--displays",
            "/does/not/exist.kdl",
            "--print-command",
        ])
        .env_remove("GREETD_SOCK")
        .env_remove("WAYLAND_DISPLAY")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let plan: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(plan["program"], "/does/not/exist");
    let argv: Vec<_> = plan["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(argv[0], "--config");
    assert!(argv[1].starts_with("/run/user/4242/canopa-greeter-"));
    let separator = argv.iter().position(|a| *a == "--").unwrap();
    assert_eq!(argv[separator + 2], "internal-report-display");
    assert_eq!(plan["client"]["argv"], serde_json::json!(["--defaults"]));
    let config = plan["niri_config"].as_str().unwrap();
    // A missing display file is never included.
    assert!(!config.contains("include"));
    assert!(
        plan["env"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!([compositor::KEEP_LAST_FRAME, "1"]))
    );
}

#[test]
fn niri_login_configuration_starts_nothing_and_binds_nothing() {
    let config = compositor::config(Rgb(0x10, 0x19, 0x1d), Some(Path::new("/etc/a \"b\".kdl")));
    for forbidden in ["spawn", "binds", "environment"] {
        assert!(!config.contains(forbidden), "{forbidden} in {config}");
    }
    assert!(config.contains("background-color \"#10191d\""));
    assert!(config.contains("xwayland-satellite { off; }"));
    assert!(config.contains("disable-power-key-handling"));
    assert!(config.contains(r#"match app-id="^org\\.canopa\\.Greeter$""#));
    assert!(config.ends_with("include \"/etc/a \\\"b\\\".kdl\"\n"));
}

#[test]
fn login_displays_must_be_root_owned_unless_explicit() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("greeter-displays.kdl");
    assert_eq!(compositor::trusted_displays(&path, false), None);
    std::fs::write(&path, "output \"X\" { off; }\n").unwrap();
    // Test files are owned by the test user, never root.
    assert_eq!(compositor::trusted_displays(&path, false), None);
    assert_eq!(
        compositor::trusted_displays(&path, true).as_deref(),
        Some(path.as_path())
    );
}

/// A stand-in Niri: runs the command after `--` with a display, as Niri spawns its
/// startup command, then waits for SIGTERM. The stand-in greeter forwards the
/// reporter to the real binary and records the display it was given.
fn fixture(directory: &Path, greeter_status: u8, niri_exits: bool) -> compositor::Niri {
    let niri = directory.join("niri");
    let wait = if niri_exits {
        "sleep 0.2; exit 3"
    } else {
        "while :; do sleep 0.02; done"
    };
    std::fs::write(
        &niri,
        format!(
            "#!/bin/sh\n\
             while [ \"$1\" != -- ]; do shift; done; shift\n\
             trap 'touch \"$MARK/niri-stopped\"; exit 0' TERM\n\
             WAYLAND_DISPLAY=wayland-test \"$@\"\n\
             {wait}\n"
        ),
    )
    .unwrap();
    let greeter = directory.join("greeter");
    std::fs::write(
        &greeter,
        format!(
            "#!/bin/sh\n\
             if [ \"$1\" = internal-report-display ]; then exec \"$REAL\" \"$@\"; fi\n\
             if [ -n \"$NIRI_EXITS\" ]; then sleep 5; fi\n\
             echo \"$WAYLAND_DISPLAY $*\" > \"$MARK/display\"\n\
             exit {greeter_status}\n"
        ),
    )
    .unwrap();
    for path in [&niri, &greeter] {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let runtime = directory.join("runtime");
    std::fs::create_dir(&runtime).unwrap();
    let mut plan = compositor::Niri::new(
        Some(&niri),
        Renderer::Auto,
        &[],
        &greeter,
        &["--defaults".into()],
        &runtime,
        Rgb(0, 0, 0),
        None,
    );
    for command in [&mut plan.compositor, &mut plan.greeter] {
        command
            .env("MARK", directory)
            .env("REAL", assert_cmd::cargo::cargo_bin!("canopa-greeter"))
            .env_remove("WAYLAND_DISPLAY");
        if niri_exits {
            command.env("NIRI_EXITS", "1");
        }
    }
    plan
}

#[test]
fn niri_runs_the_greeter_on_its_display_and_stops_with_it() {
    let directory = tempfile::tempdir().unwrap();
    let plan = fixture(directory.path(), 0, false);
    let runtime = plan.runtime.clone();
    plan.run().unwrap();
    assert_eq!(
        std::fs::read_to_string(directory.path().join("display")).unwrap(),
        "wayland-test --defaults\n"
    );
    assert!(directory.path().join("niri-stopped").exists());
    assert!(!runtime.exists(), "runtime directory removed");
}

#[test]
fn a_failed_greeter_or_compositor_is_an_error() {
    let directory = tempfile::tempdir().unwrap();
    let error = fixture(directory.path(), 1, false).run().unwrap_err();
    assert!(format!("{error:#}").contains("exit status 1"), "{error:#}");
    assert!(directory.path().join("niri-stopped").exists());

    let directory = tempfile::tempdir().unwrap();
    let error = fixture(directory.path(), 0, true).run().unwrap_err();
    assert!(format!("{error:#}").contains("Niri exited"), "{error:#}");
    assert!(!directory.path().join("display").exists());
}
