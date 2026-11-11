use std::{
    fs,
    process::{Command, Output},
};

fn check(args: &[&str], config_home: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canopa-shell"))
        .arg("--check-config")
        .env("CANOPA_LANGUAGE", "en")
        .args(args)
        .env("XDG_CONFIG_HOME", config_home)
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .output()
        .unwrap()
}
fn report(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn shell_checks_use_discovery_and_namespace_isolation_without_a_desktop() {
    let temp = tempfile::tempdir().unwrap();
    let default = check(&[], temp.path());
    assert!(
        default.status.success(),
        "{}",
        String::from_utf8_lossy(&default.stderr)
    );
    assert_eq!(report(&default)["source"]["defaults"], true);
    fs::create_dir(temp.path().join("niribar")).unwrap();
    fs::write(
        temp.path().join("niribar/config.kdl"),
        "bar \"legacy\" {}\n",
    )
    .unwrap();
    let legacy = check(&[], temp.path());
    assert!(legacy.status.success());
    assert_eq!(report(&legacy)["source"]["format"], "legacy");
    fs::create_dir(temp.path().join("canopa")).unwrap();
    let common = temp.path().join("canopa/config.kdl");
    fs::write(
        &common,
        "schema-version 1\nshell {}\nnotifications { invalid-property 42; }\n",
    )
    .unwrap();
    let good = check(&[], temp.path());
    assert!(good.status.success());
    assert_eq!(report(&good)["source"]["format"], "canopa-v1");
    let text = "schema-version 1\nnotifications {}\nshell { private-invalid-option 42; }\n";
    fs::write(&common, text).unwrap();
    let bad = check(&[], temp.path());
    assert!(!bad.status.success());
    assert_eq!(report(&bad)["error"]["code"], "schema");
    assert_eq!(
        report(&bad)["error"]["offset"],
        text.find("private-invalid-option").unwrap()
    );
    assert!(!String::from_utf8_lossy(&bad.stdout).contains("private-invalid-option"));
    assert!(!String::from_utf8_lossy(&bad.stderr).contains("private-invalid-option"));
    let rich = check(&["--show-source"], temp.path());
    assert!(String::from_utf8_lossy(&rich.stderr).contains("private-invalid-option"));
    fs::write(&common, "schema-version 1\nnotifications {}\n").unwrap();
    let missing = check(&[], temp.path());
    assert!(missing.status.success());
    assert_eq!(report(&missing)["source"]["defaults"], true);
}

#[test]
fn shell_sources_are_explicit_and_admission_is_bounded() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("config.kdl");
    fs::write(&file, "bar \"main\" {}\n").unwrap();
    assert!(
        check(&["--config", file.to_str().unwrap()], temp.path())
            .status
            .success()
    );
    assert!(
        !check(&["--canopa-config", file.to_str().unwrap()], temp.path())
            .status
            .success()
    );
    fs::write(&file, " ".repeat(canopa_config::MAX_BYTES + 1)).unwrap();
    let output = check(&["--config", file.to_str().unwrap()], temp.path());
    assert_eq!(report(&output)["error"]["code"], "read");
    assert!(
        !check(
            &["--defaults", "--config", file.to_str().unwrap()],
            temp.path()
        )
        .status
        .success()
    );
}

#[test]
fn source_details_are_explicit_and_syntax_diagnostics_keep_original_text() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("syntax.kdl");
    fs::write(&file, "schema-version 1\nshell { private-syntax {\n").unwrap();
    let args = ["--canopa-config", file.to_str().unwrap()];
    let safe = check(&args, temp.path());
    assert_eq!(report(&safe)["error"]["code"], "syntax");
    assert!(!String::from_utf8_lossy(&safe.stdout).contains("private-syntax"));
    assert!(!String::from_utf8_lossy(&safe.stderr).contains("private-syntax"));
    let rich = check(&[args[0], args[1], "--show-source"], temp.path());
    assert_eq!(report(&rich)["error"]["code"], "syntax");
    assert!(String::from_utf8_lossy(&rich.stderr).contains("private-syntax"));
}
