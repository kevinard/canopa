//! Test-owned libc provider. Never changes host NSS configuration or account records.
use crate::nss_support as support;
use serde_json::Value;
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

fn query(root: &Path, library: &Path, mode: &str, settings: &str) -> assert_cmd::Command {
    let config = root.join("greeter.kdl");
    fs::write(&config, format!("schema-version 1\ngreeter {{ user-source \"nss\"; nss-uid-min 1000; nss-uid-max 300000; {settings} }}\n")).unwrap();
    let mut command = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"));
    command.env("CANOPA_LANGUAGE", "en");
    command
        .args(["--canopa-config"])
        .arg(config)
        .arg("list-users")
        .env(
            "DBUS_SYSTEM_BUS_ADDRESS",
            "unix:path=/nonexistent-canopa-test-bus",
        )
        .timeout(Duration::from_secs(5));
    support::configure(&mut command, root, library, mode);
    command
}
fn users(command: &mut assert_cmd::Command) -> Vec<Value> {
    serde_json::from_slice::<Vec<Value>>(&command.assert().success().get_output().stdout).unwrap()
}

#[test]
fn nss_filters_records_preserves_canonical_identity_and_limits_gecos() {
    let root = tempfile::tempdir().unwrap();
    let lib = support::fixture(root.path());
    let result = users(&mut query(
        root.path(),
        &lib,
        "ok",
        "users \"bob\" \"manual\";",
    ));
    assert_eq!(
        result
            .iter()
            .map(|r| r["username"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["bob", "manual", "alice", "enterprise", "fallback"]
    );
    assert_eq!(result[0]["display_name"], "Bob NSS");
    assert_eq!(result[0]["uid"], 1200);
    assert_eq!(result[2]["display_name"], "Alice NSS");
    assert_eq!(result[4]["display_name"], "fallback");
    let text = serde_json::to_string(&result).unwrap();
    for private in ["555-", "Room", "PASSWORD", "HOME", "pw_shell"] {
        assert!(!text.contains(private));
    }
    support::no_live_helpers(root.path());
    let result = users(&mut query(
        root.path(),
        &lib,
        "ok",
        "nss-include-nologin #true;",
    ));
    assert!(result.iter().any(|r| r["username"] == "disabled"));
    assert!(result.iter().any(|r| r["username"] == "false-shell"));
    assert!(!result.iter().any(|r| r["username"] == "root"));
    let mut cmd = query(root.path(), &lib, "ok", "");
    fs::write(
        root.path().join("greeter.kdl"),
        "schema-version 1\ngreeter { user-source \"nss\"; nss-uid-min 1100; nss-uid-max 1300; }\n",
    )
    .unwrap();
    assert_eq!(
        users(&mut cmd)
            .iter()
            .map(|r| r["username"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["bob"]
    );
}

#[test]
fn nss_is_bounded_and_failures_retain_manual_users_and_reap_helpers() {
    let root = tempfile::tempdir().unwrap();
    let lib = support::fixture(root.path());
    assert_eq!(users(&mut query(root.path(), &lib, "many", "")).len(), 128);
    for mode in ["error", "range", "budget", "overflow", "block", "fork"] {
        fs::remove_file(root.path().join("nss-events")).unwrap();
        let started = Instant::now();
        let result = users(&mut query(root.path(), &lib, mode, "users \"manual\";"));
        assert!(started.elapsed() < Duration::from_secs(4), "{mode}");
        assert_eq!(result.len(), 1, "{mode}");
        assert_eq!(result[0]["username"], "manual");
        support::no_live_helpers(root.path());
    }
}

#[test]
fn malformed_nss_configuration_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("greeter.kdl");
    for settings in [
        "user-source \"unknown\";",
        "user-source #true;",
        "nss-uid-min -1;",
        "nss-uid-max 4294967295;",
        "nss-uid-min 2000; nss-uid-max 1000;",
        "nss-uid-min \"1000\";",
        "nss-include-nologin \"true\";",
        "nss-uid-min 1000; nss-uid-min 1001;",
    ] {
        fs::write(
            &config,
            format!("schema-version 1\ngreeter {{ {settings} }}\n"),
        )
        .unwrap();
        assert!(
            canopa_greeter::config::Config::load(&canopa_config::Request::common(&config)).is_err(),
            "{settings}"
        );
    }
}

#[test]
fn helper_dies_if_its_frontend_disappears() {
    use std::process::{Command, Stdio};
    let root = tempfile::tempdir().unwrap();
    let lib = support::fixture(root.path());
    let mut parent = Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"))
        .env("CANOPA_LANGUAGE", "en")
        .args(["--defaults", "list-users", "--source", "nss"])
        .env("LD_PRELOAD", lib)
        .env("CANOPA_NSS_FIXTURE", "block")
        .env("CANOPA_NSS_EVENTS", root.path().join("nss-events"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while !root.path().join("nss-events").exists() {
        assert!(started.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(10));
    }
    parent.kill().unwrap();
    parent.wait().unwrap();
    let pid = fs::read_to_string(root.path().join("nss-events"))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();
    for _ in 0..100 {
        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
        if stat.is_empty() || stat.contains(") Z ") {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("orphan NSS helper survived frontend death");
}
