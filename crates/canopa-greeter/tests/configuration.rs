use canopa_config::Request;
use canopa_greeter::config::Config;
#[test]
fn system_source_does_not_follow_user_discovery_and_preview_runs_no_commands() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("config.kdl");
    let marker = dir.path().join("ran");
    let source = format!(
        r##"schema-version 1
greeter {{
 users "alice" "bob"
 default-user "bob"
 session "canopa" {{ label "Canopa"; command {{ argv "niri-session"; }}; env "XDG_SESSION_TYPE=wayland"; }}
 view {{
  widgets "status"
  layout {{ top-left "clock" "date"; center "authentication"; bottom-right "status"; }}
  widget "status" {{ command {{ argv "touch" "{}"; }}; preview "A greeter widget"; }}
 }}
}}
"##,
        marker.display()
    );
    std::fs::write(&file, &source).unwrap();
    let (cfg, _) = Config::load(&Request::common(&file)).unwrap();
    assert_eq!(cfg.default_user, "bob");
    assert_eq!(cfg.sessions[0].argv, ["niri-session"]);
    for command in ["check-config", "preview"] {
        let mut app = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"));
        app.arg("--canopa-config").arg(&file).arg(command);
        if command == "preview" {
            app.arg(dir.path().join("preview.png"));
        }
        app.env_remove("GREETD_SOCK")
            .env_remove("WAYLAND_DISPLAY")
            .assert()
            .success();
        assert!(!marker.exists());
    }
    for (old, new) in [
        ("users \"alice\" \"bob\"", "users \"alice\" \"alice\""),
        ("default-user \"bob\"", "default-user \"bad user\""),
        ("argv \"niri-session\"", "argv"),
        ("XDG_SESSION_TYPE=wayland", "=invalid"),
        ("center \"authentication\"", "center \"clock\""),
        ("widgets \"status\"", "widgets \"media\""),
    ] {
        std::fs::write(&file, source.replace(old, new)).unwrap();
        assert!(Config::load(&Request::common(&file)).is_err(), "{new}");
    }
    // Individually valid arguments must also fit greetd's internal worker frame.
    let oversized = source.replace(
        "argv \"niri-session\"",
        &format!(
            "argv \"niri-session\" \"{}\" \"{}\"",
            "x".repeat(4096),
            "y".repeat(4096)
        ),
    );
    std::fs::write(&file, oversized).unwrap();
    assert!(
        Config::load(&Request::common(&file))
            .unwrap_err()
            .to_string()
            .contains("backend budget")
    );
    // A bad user-local shared file must not affect the normal system-config path.
    let home = dir.path().join("user-config/canopa");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("config.kdl"), "invalid").unwrap();
    let mut app = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"));
    app.arg("--defaults")
        .arg("check-config")
        .env("XDG_CONFIG_HOME", home.parent().unwrap())
        .assert()
        .success();
}

#[test]
fn greetd_shell_launch_preserves_spaces_quotes_empty_args_and_metacharacters() {
    let directory = tempfile::tempdir().unwrap();
    let marker = directory.path().join("must-not-run");
    let literal = format!("$(touch {})", marker.display());
    let session = canopa_greeter::config::Session {
        id: "test".into(),
        label: "Test".into(),
        argv: vec![
            "printf".into(),
            "%s\n".into(),
            "arg with spaces".into(),
            "'quoted'".into(),
            literal.clone(),
            String::new(),
        ],
        env: Vec::new(),
        comment: None,
        desktop_file: None,
    };
    // Match greetd's actual shell launch rather than treating its array as execve argv.
    let result = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("exec {}", session.shell_command().unwrap()))
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        format!("arg with spaces\n'quoted'\n{literal}\n\n")
    );
    assert!(!marker.exists());
}
