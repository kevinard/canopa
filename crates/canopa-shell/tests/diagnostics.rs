use std::process::Command;

#[test]
fn renamed_performance_switch_takes_precedence_over_legacy_environment() {
    for (current, legacy, enabled) in [
        (None, None, false),
        (None, Some("1"), true),
        (Some("1"), None, true),
        (Some("0"), Some("1"), false),
        (Some("1"), Some("0"), true),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_canopa-shell"));
        command
            .arg("--version")
            .env("RUST_LOG", "canopa_shell::diagnostics=debug")
            .env_remove("CANOPA_SHELL_PERF")
            .env_remove("NIRIBAR_PERF");
        if let Some(value) = current {
            command.env("CANOPA_SHELL_PERF", value);
        }
        if let Some(value) = legacy {
            command.env("NIRIBAR_PERF", value);
        }
        let output = command.output().unwrap();
        let diagnostics = String::from_utf8(output.stderr).unwrap();
        assert!(output.status.success(), "{diagnostics}");
        assert!(
            diagnostics.contains(&format!("perf_enabled={enabled}")),
            "current={current:?}, legacy={legacy:?}: {diagnostics}"
        );
    }
}
