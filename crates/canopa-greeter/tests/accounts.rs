//! Independent private-bus fixture; no host account data or policy is consulted.
use crate::nss_support as nss;
use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Bus {
    process: Child,
    address: String,
}
impl Bus {
    fn start() -> Self {
        use std::io::{BufRead, BufReader};
        let mut process = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut address = String::new();
        BufReader::new(process.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        Self {
            process,
            address: address.trim().into(),
        }
    }
}
impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
struct Service(Child);
impl Service {
    fn start(bus: &Bus, root: &Path) -> Self {
        let script =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/desktop-fixture/accounts.py");
        let child = Command::new("/usr/bin/python3")
            .arg(script)
            .arg(root)
            .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        while !root.join("accounts-ready").exists() {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "private AccountsService did not start"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        Self(child)
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn query(config: &Path, bus: &Bus) -> assert_cmd::Command {
    query_source(config, bus, "accountsservice")
}
fn query_source(config: &Path, bus: &Bus, source: &str) -> assert_cmd::Command {
    let mut command = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"));
    command
        .arg("--canopa-config")
        .arg(config)
        .arg("list-users")
        .args(["--source", source])
        .env("DBUS_SYSTEM_BUS_ADDRESS", &bus.address)
        .env_remove("GREETD_SOCK")
        .env_remove("WAYLAND_DISPLAY")
        .timeout(Duration::from_secs(5));
    command
}

#[test]
fn automatic_source_prefers_accountsservice_and_only_falls_back_on_failure() {
    let root = tempfile::tempdir().unwrap();
    let lib = nss::fixture(root.path());
    let bus = Bus::start();
    let config = root.path().join("greeter.kdl");
    fs::write(
        &config,
        "schema-version 1\ngreeter { nss-uid-min 1000; nss-uid-max 300000; }\n",
    )
    .unwrap();
    let inspect = |source: &str| {
        let mut command = query_source(&config, &bus, source);
        nss::configure(&mut command, root.path(), &lib, "ok");
        serde_json::from_slice::<Vec<Value>>(&command.assert().success().get_output().stdout)
            .unwrap()
    };
    assert!(
        inspect("auto")
            .iter()
            .any(|u| u["display_name"] == "Alice NSS")
    );
    nss::no_live_helpers(root.path());
    fs::remove_file(root.path().join("nss-events")).unwrap();
    let _service = Service::start(&bus, root.path());
    assert!(
        inspect("auto")
            .iter()
            .any(|u| u["display_name"] == "Alice Example")
    );
    assert!(!root.path().join("nss-events").exists());
    fs::write(
        root.path().join("accounts-control.json"),
        r#"{"mode":"empty"}"#,
    )
    .unwrap();
    assert!(inspect("auto").is_empty());
    assert!(!root.path().join("nss-events").exists());
    for settings in [r#"{"mode":"denied"}"#, r#"{"delay_ms":3000}"#] {
        fs::write(root.path().join("accounts-control.json"), settings).unwrap();
        assert!(
            inspect("auto")
                .iter()
                .any(|u| u["display_name"] == "Alice NSS")
        );
        nss::no_live_helpers(root.path());
    }
    fs::remove_file(root.path().join("nss-events")).unwrap();
    let before = fs::read(root.path().join("accounts-events.json")).unwrap();
    assert!(
        inspect("nss")
            .iter()
            .any(|u| u["display_name"] == "Alice NSS")
    );
    assert_eq!(
        fs::read(root.path().join("accounts-events.json")).unwrap(),
        before
    );
    fs::remove_file(root.path().join("nss-events")).unwrap();
    fs::write(
        &config,
        "schema-version 1\ngreeter { discover-users #false; users \"manual\"; }\n",
    )
    .unwrap();
    let users = inspect("auto");
    assert_eq!(users.len(), 1);
    assert_eq!(users[0]["username"], "manual");
    assert!(!root.path().join("nss-events").exists());
    assert_eq!(
        fs::read(root.path().join("accounts-events.json")).unwrap(),
        before
    );
}

#[test]
fn avatar_metadata_is_optional_and_disabling_avatars_avoids_icon_queries() {
    let root = tempfile::tempdir().unwrap();
    let bus = Bus::start();
    let _service = Service::start(&bus, root.path());
    let config = root.path().join("greeter.kdl");
    fs::write(&config, "schema-version 1\ngreeter {}\n").unwrap();
    let inspect = || {
        serde_json::from_slice::<Vec<Value>>(
            &query(&config, &bus).assert().success().get_output().stdout,
        )
        .unwrap()
    };
    let users = inspect();
    let alice = users.iter().find(|u| u["username"] == "alice").unwrap();
    assert_eq!(
        alice["icon_file"],
        root.path().join("avatar-alice").to_str().unwrap()
    );
    for mode in ["missing", "invalid"] {
        fs::write(
            root.path().join("accounts-control.json"),
            serde_json::json!({"icon_mode":mode}).to_string(),
        )
        .unwrap();
        let users = inspect();
        let alice = users.iter().find(|u| u["username"] == "alice").unwrap();
        assert!(alice.get("icon_file").is_none());
        assert_eq!(alice["display_name"], "Alice Example");
    }
    fs::write(
        &config,
        "schema-version 1\ngreeter { user-avatars #false; }\n",
    )
    .unwrap();
    let before: Vec<Value> =
        serde_json::from_slice(&fs::read(root.path().join("accounts-events.json")).unwrap())
            .unwrap();
    assert!(inspect().iter().all(|u| u.get("icon_file").is_none()));
    let after: Vec<Value> =
        serde_json::from_slice(&fs::read(root.path().join("accounts-events.json")).unwrap())
            .unwrap();
    assert!(
        after[before.len()..]
            .iter()
            .all(|e| e["name"] != "IconFile")
    );
}

#[test]
fn accountsservice_discovers_cached_users_with_read_only_queries_and_manual_priority() {
    let root = tempfile::tempdir().unwrap();
    let bus = Bus::start();
    let _service = Service::start(&bus, root.path());
    let config = root.path().join("greeter.kdl");
    fs::write(
        &config,
        "schema-version 1\ngreeter { users \"remote\" \"manual\"; }\n",
    )
    .unwrap();
    let output = query(&config, &bus)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let accounts: Value = serde_json::from_slice(&output).unwrap();
    let accounts = accounts.as_array().unwrap();
    assert_eq!(accounts[0]["username"], "remote");
    assert_eq!(accounts[0]["display_name"], "Remote User");
    assert_eq!(accounts[1]["username"], "manual");
    assert!(accounts[1]["uid"].is_null());
    assert!(
        accounts
            .iter()
            .any(|a| a["username"] == "alice" && a["display_name"] == "Alice Example")
    );
    assert!(accounts.iter().any(|a| a["username"] == "locked"));
    assert!(
        accounts
            .iter()
            .any(|a| a["username"] == "fallback" && a["display_name"] == "fallback")
    );
    assert!(
        accounts
            .iter()
            .any(|a| a["username"] == "missing" && a["display_name"] == "missing")
    );
    assert!(!accounts.iter().any(|a| {
        ["root", "daemon", "bad user"]
            .iter()
            .any(|n| a["username"] == *n)
    }));
    let events: Vec<Value> =
        serde_json::from_slice(&fs::read(root.path().join("accounts-events.json")).unwrap())
            .unwrap();
    assert_eq!(events.iter().filter(|e| e["event"] == "method").count(), 1);
    assert!(events.iter().filter(|e| e["event"] == "property").all(|e| {
        ["Uid", "UserName", "RealName", "SystemAccount", "IconFile"]
            .iter()
            .any(|name| e["name"] == *name)
    }));
}

#[test]
fn disabled_discovery_and_schema_preview_do_not_contact_the_service() {
    let root = tempfile::tempdir().unwrap();
    let bus = Bus::start();
    let _service = Service::start(&bus, root.path());
    let config = root.path().join("greeter.kdl");
    fs::write(
        &config,
        "schema-version 1\ngreeter { users \"alice\"; discover-users #false; }\n",
    )
    .unwrap();
    let result = query(&config, &bus)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let data: Value = serde_json::from_slice(&result).unwrap();
    assert_eq!(data.as_array().unwrap().len(), 1);
    assert!(data[0]["uid"].is_null());
    fs::write(
        &config,
        "schema-version 1\ngreeter { discover-users #true; }\n",
    )
    .unwrap();
    for action in ["check-config", "preview"] {
        let mut command = assert_cmd::Command::new(assert_cmd::cargo::cargo_bin!("canopa-greeter"));
        command
            .arg("--canopa-config")
            .arg(&config)
            .arg(action)
            .env("DBUS_SYSTEM_BUS_ADDRESS", &bus.address);
        if action == "preview" {
            command.arg(root.path().join("preview.png"));
        }
        command.assert().success();
    }
    let events: Vec<Value> =
        serde_json::from_slice(&fs::read(root.path().join("accounts-events.json")).unwrap())
            .unwrap();
    assert!(
        !events
            .iter()
            .any(|e| e["event"] == "method" || e["event"] == "property")
    );
    fs::write(
        &config,
        "schema-version 1\ngreeter { discover-users \"true\"; }\n",
    )
    .unwrap();
    assert!(
        canopa_greeter::config::Config::load(&canopa_config::Request::common(&config)).is_err()
    );
}

#[test]
fn absent_denied_oversized_and_slow_services_keep_manual_entry_available() {
    let root = tempfile::tempdir().unwrap();
    let bus = Bus::start();
    let config = root.path().join("greeter.kdl");
    fs::write(&config, "schema-version 1\ngreeter { users \"manual\"; }\n").unwrap();
    fn manual(data: &[u8]) {
        let users: Value = serde_json::from_slice(data).unwrap();
        assert_eq!(users.as_array().unwrap().len(), 1);
        assert_eq!(users[0]["username"], "manual");
    }
    manual(&query(&config, &bus).assert().success().get_output().stdout);
    let _service = Service::start(&bus, root.path());
    for settings in [
        serde_json::json!({"mode":"denied"}),
        serde_json::json!({"mode":"oversized"}),
        serde_json::json!({"delay_ms":3000}),
    ] {
        fs::write(
            root.path().join("accounts-control.json"),
            settings.to_string(),
        )
        .unwrap();
        let started = Instant::now();
        manual(&query(&config, &bus).assert().success().get_output().stdout);
        assert!(started.elapsed() < Duration::from_secs(4));
    }
}
