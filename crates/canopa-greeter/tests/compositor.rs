use canopa_greeter::compositor::{self, Compositor, Renderer};
use std::{ffi::OsString, path::Path};

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
