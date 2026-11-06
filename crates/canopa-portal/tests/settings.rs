use canopa_portal::{BUS_NAME, NAMESPACE, OBJECT_PATH};
use futures_util::StreamExt;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};
use zbus::zvariant::OwnedValue;

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}
fn backend(address: &str, config_home: &Path, log: &Path) -> Process {
    Process(
        Command::new(env!("CARGO_BIN_EXE_xdg-desktop-portal-canopa"))
            .env("DBUS_SESSION_BUS_ADDRESS", address)
            .env("XDG_CONFIG_HOME", config_home)
            .env("XDG_STATE_HOME", config_home.join("state"))
            .env_remove("NIRI_SOCKET")
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(log).unwrap())
            .spawn()
            .unwrap(),
    )
}
async fn wait_ready(proxy: &zbus::Proxy<'_>, child: &mut Process, log: &Path) {
    let mut last_error = String::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "backend exited before readiness"
            );
            match proxy.get_property::<u32>("version").await {
                Ok(_) => return,
                Err(error) => last_error = error.to_string(),
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|error| {
        panic!(
            "backend readiness: {error}; last D-Bus error: {last_error}; startup: {}",
            std::fs::read_to_string(log).unwrap()
        )
    });
}
async fn reload(child: &Process, log: &Path, event: &str) {
    // The content-free completion diagnostic is a barrier for rejected/no-op reloads.
    let previous = std::fs::read_to_string(log).unwrap().matches(event).count();
    assert!(
        Command::new("kill")
            .args(["-HUP", &child.0.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        while std::fs::read_to_string(log).unwrap().matches(event).count() == previous {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
async fn read(proxy: &zbus::Proxy<'_>, key: &str) -> OwnedValue {
    proxy.call("Read", &(NAMESPACE, key)).await.unwrap()
}

#[tokio::test]
async fn backend_wire_reload_discovery_and_restart_contract() {
    let dir = tempfile::tempdir().unwrap();
    let bus_config = dir.path().join("bus.conf");
    // No installed activation services: only this fixture's explicit child can own the name.
    std::fs::write(&bus_config, "<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><auth>EXTERNAL</auth><policy context=\"default\"><allow send_destination=\"*\"/><allow receive_sender=\"*\"/><allow own=\"*\"/></policy></busconfig>").unwrap();
    let mut bus = Process(
        Command::new("dbus-daemon")
            .args(["--nofork", "--print-address=1", "--config-file"])
            .arg(&bus_config)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut address = String::new();
    BufReader::new(bus.0.stdout.take().unwrap())
        .read_line(&mut address)
        .unwrap();
    let address = address.trim();
    let connection = zbus::connection::Builder::address(address)
        .unwrap()
        .build()
        .await
        .unwrap();
    let config = dir.path().join("config");
    std::fs::create_dir_all(config.join("niribar")).unwrap();
    std::fs::write(
        config.join("niribar/config.kdl"),
        "ignored legacy-private-value",
    )
    .unwrap();
    let log = dir.path().join("backend.log");
    let mut child = backend(address, &config, &log);
    // Poll the service, not a cached pre-activation GetAll error or old-owner value.
    let proxy: zbus::Proxy<'_> = zbus::proxy::Builder::new(&connection)
        .destination(BUS_NAME)
        .unwrap()
        .path(OBJECT_PATH)
        .unwrap()
        .interface("org.freedesktop.impl.portal.Settings")
        .unwrap()
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await
        .unwrap();
    wait_ready(&proxy, &mut child, &log).await;
    assert_eq!(proxy.get_property::<u32>("version").await.unwrap(), 1);
    assert_eq!(
        u32::try_from(read(&proxy, "color-scheme").await).unwrap(),
        1
    );
    let accent: (f64, f64, f64) = read(&proxy, "accent-color").await.try_into().unwrap();
    assert_eq!(accent, (176.0 / 255.0, 222.0 / 255.0, 195.0 / 255.0));
    for filter in [
        vec![],
        vec![""],
        vec!["*"],
        vec!["org.*"],
        vec!["org.freedesktop.*"],
        vec![NAMESPACE],
        vec!["unknown", NAMESPACE],
    ] {
        let all: HashMap<String, HashMap<String, OwnedValue>> =
            proxy.call("ReadAll", &(filter,)).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[NAMESPACE].len(), 2);
        assert_eq!(u32::try_from(&all[NAMESPACE]["color-scheme"]).unwrap(), 1);
    }
    for filter in [
        "org.freedesktop",
        "org.freedesktop.appearance.*",
        "org.*.appearance",
        "unknown",
    ] {
        let all: HashMap<String, HashMap<String, OwnedValue>> =
            proxy.call("ReadAll", &(vec![filter],)).await.unwrap();
        assert!(all.is_empty());
    }
    for (namespace, key) in [
        ("private-namespace", "color-scheme"),
        (NAMESPACE, "private-key"),
        (NAMESPACE, "contrast"),
    ] {
        let error = proxy
            .call::<_, _, OwnedValue>("Read", &(namespace, key))
            .await
            .unwrap_err();
        assert!(
            matches!(error, zbus::Error::MethodError(ref name, _, _) if name.as_str() == "org.freedesktop.portal.Error.NotFound")
        );
        assert!(!error.to_string().contains("private-"));
    }
    let mut signals = proxy.receive_signal("SettingChanged").await.unwrap();
    std::fs::create_dir(config.join("canopa")).unwrap();
    let file = config.join("canopa/config.kdl");
    std::fs::write(&file, "schema-version 1\nappearance { color-scheme \"light\"; accent \"#123456\"; }\nshell { private-unrelated 1; }").unwrap();
    reload(&child, &log, "event=reloaded").await;
    let mut changed = HashMap::new();
    for _ in 0..2 {
        let signal = tokio::time::timeout(Duration::from_secs(2), signals.next())
            .await
            .unwrap()
            .unwrap();
        let (namespace, key, value): (String, String, OwnedValue) =
            signal.body().deserialize().unwrap();
        assert_eq!(namespace, NAMESPACE);
        changed.insert(key, value);
    }
    assert_eq!(u32::try_from(&changed["color-scheme"]).unwrap(), 2);
    let accent: (f64, f64, f64) = changed.remove("accent-color").unwrap().try_into().unwrap();
    assert_eq!(accent, (18.0 / 255.0, 52.0 / 255.0, 86.0 / 255.0));
    assert_eq!(
        u32::try_from(read(&proxy, "color-scheme").await).unwrap(),
        2
    );
    reload(&child, &log, "event=reloaded").await;
    assert!(
        tokio::time::timeout(Duration::from_millis(100), signals.next())
            .await
            .is_err()
    );
    std::fs::write(
        &file,
        "schema-version 1\nappearance { color-scheme \"private-value\"; }",
    )
    .unwrap();
    reload(&child, &log, "event=reload_rejected").await;
    assert_eq!(
        u32::try_from(read(&proxy, "color-scheme").await).unwrap(),
        2
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), signals.next())
            .await
            .is_err()
    );
    assert!(
        !std::fs::read_to_string(&log)
            .unwrap()
            .contains("private-value")
    );
    // A second backend must not replace the current owner.
    std::fs::write(
        &file,
        "schema-version 1\nappearance { color-scheme \"dark\"; }",
    )
    .unwrap();
    let mut duplicate = backend(address, &config, &dir.path().join("duplicate.log"));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(status) = duplicate.0.try_wait().unwrap() {
                assert!(!status.success());
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        u32::try_from(read(&proxy, "color-scheme").await).unwrap(),
        2
    );
    drop(signals);
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    let mut child = backend(address, &config, &log);
    wait_ready(&proxy, &mut child, &log).await;
    assert_eq!(
        u32::try_from(read(&proxy, "color-scheme").await).unwrap(),
        1
    );
    assert!(
        Command::new("kill")
            .args(["-TERM", &child.0.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    std::fs::write(
        &file,
        "schema-version 1\nappearance { accent \"private-invalid\"; }",
    )
    .unwrap();
    let mut invalid = backend(address, &config, &log);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(status) = invalid.0.try_wait().unwrap() {
                assert!(!status.success());
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        !std::fs::read_to_string(&log)
            .unwrap()
            .contains("private-invalid")
    );
    std::fs::remove_file(&file).unwrap();
    let mut child = backend(address, &config, &log);
    wait_ready(&proxy, &mut child, &log).await;
    bus.0.kill().unwrap();
    bus.0.wait().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while child.0.try_wait().unwrap().is_none() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
