use canopa_portal::{
    BUS_NAME, OBJECT_PATH,
    lockdown::{INTERFACE, Lockdown, Source},
};
use futures_util::StreamExt;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};
use zbus::zvariant::{OwnedValue, Value};

const KEYS: [&str; 7] = [
    "disable-printing",
    "disable-save-to-disk",
    "disable-application-handlers",
    "disable-location",
    "disable-camera",
    "disable-microphone",
    "disable-sound-output",
];
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        self.0.kill().ok();
        self.0.wait().ok();
    }
}
async fn properties(connection: &zbus::Connection) -> zbus::Proxy<'_> {
    zbus::Proxy::new(
        connection,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.DBus.Properties",
    )
    .await
    .unwrap()
}
async fn all(proxy: &zbus::Proxy<'_>) -> HashMap<String, OwnedValue> {
    proxy.call("GetAll", &(INTERFACE,)).await.unwrap()
}
async fn get(proxy: &zbus::Proxy<'_>, key: &str) -> bool {
    let result: OwnedValue = proxy.call("Get", &(INTERFACE, key)).await.unwrap();
    bool::try_from(result).unwrap()
}
async fn set(proxy: &zbus::Proxy<'_>, key: &str, value: bool) -> zbus::Result<()> {
    proxy
        .call("Set", &(INTERFACE, key, Value::from(value)))
        .await
}
fn backend(address: &str, root: &Path, readonly: bool) -> Process {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xdg-desktop-portal-canopa"));
    command
        .args(["--defaults", "--lockdown-policy"])
        .arg(root.join("policy.sqlite3"));
    if readonly {
        command.arg("--lockdown-read-only");
    }
    Process(
        command
            .env("DBUS_SESSION_BUS_ADDRESS", address)
            .env("CANOPA_LANGUAGE", "en")
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_STATE_HOME", root.join("state"))
            .env_remove("NIRI_SOCKET")
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(root.join("backend.log")).unwrap())
            .spawn()
            .unwrap(),
    )
}
async fn ready(proxy: &zbus::Proxy<'_>, child: &mut Process) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            assert!(child.0.try_wait().unwrap().is_none());
            if proxy
                .call::<_, _, HashMap<String, OwnedValue>>("GetAll", &(INTERFACE,))
                .await
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
async fn stop(child: &mut Process) {
    assert!(
        Command::new("kill")
            .args(["-TERM", &child.0.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    tokio::time::timeout(Duration::from_secs(5), async {
        while child.0.try_wait().unwrap().is_none() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn persisted_policy_has_exact_properties_conditional_signals_and_real_daemon_restart() {
    let root = tempfile::tempdir().unwrap();
    let bus_config = root.path().join("bus.conf");
    std::fs::write(&bus_config, format!("<busconfig><type>session</type><listen>unix:tmpdir={}</listen><policy context=\"default\"><allow send_destination=\"*\"/><allow receive_sender=\"*\"/><allow own=\"*\"/></policy></busconfig>", root.path().display())).unwrap();
    let mut bus = Process(
        Command::new("dbus-daemon")
            .args(["--nofork", "--print-address=1", "--config-file"])
            .arg(bus_config)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut address = String::new();
    BufReader::new(bus.0.stdout.take().unwrap())
        .read_line(&mut address)
        .unwrap();
    let address = address.trim();
    let policy = Lockdown::open(Source::file(root.path().join("policy.sqlite3"), false).unwrap())
        .await
        .unwrap();
    let server = zbus::connection::Builder::address(address)
        .unwrap()
        .serve_at(OBJECT_PATH, policy.clone())
        .unwrap()
        .name(BUS_NAME)
        .unwrap()
        .build()
        .await
        .unwrap();
    let published = policy.clone();
    let connection = server.clone();
    let relay = tokio::spawn(async move { published.relay(connection).await });
    let client = zbus::connection::Builder::address(address)
        .unwrap()
        .build()
        .await
        .unwrap();
    let proxy = properties(&client).await;
    let xml: String = zbus::Proxy::new(
        &client,
        BUS_NAME,
        OBJECT_PATH,
        "org.freedesktop.DBus.Introspectable",
    )
    .await
    .unwrap()
    .call("Introspect", &())
    .await
    .unwrap();
    let section = format!("<interface name=\"{INTERFACE}\">");
    let xml = xml
        .split(section.as_str())
        .nth(1)
        .unwrap()
        .split("</interface>")
        .next()
        .unwrap();
    assert_eq!(xml.matches("<property").count(), 7);
    assert!(!xml.contains("version"));
    assert!(!xml.contains("<method"));
    for key in KEYS {
        assert!(
            xml.contains(&format!("name=\"{key}\" type=\"b\" access=\"readwrite\"")),
            "{key}: {xml}"
        );
    }
    let initial = all(&proxy).await;
    assert_eq!(initial.len(), 7);
    assert!(
        initial
            .values()
            .all(|value| !bool::try_from(value).unwrap())
    );
    assert!(!root.path().join("policy.sqlite3").exists());
    let mut changes = proxy.receive_signal("PropertiesChanged").await.unwrap();
    set(&proxy, KEYS[0], false).await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(100), changes.next())
            .await
            .is_err()
    );
    assert!(!root.path().join("policy.sqlite3").exists());
    for key in KEYS {
        set(&proxy, key, true).await.unwrap();
        let event = tokio::time::timeout(Duration::from_secs(3), changes.next())
            .await
            .unwrap()
            .unwrap();
        let (interface, values, invalidated): (String, HashMap<String, OwnedValue>, Vec<String>) =
            event.body().deserialize().unwrap();
        assert_eq!(interface, INTERFACE);
        assert!(invalidated.is_empty());
        assert_eq!(values.len(), 1);
        assert!(bool::try_from(&values[key]).unwrap());
        assert!(get(&proxy, key).await);
    }
    set(&proxy, KEYS[0], true).await.unwrap();
    policy.reload().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(100), changes.next())
            .await
            .is_err()
    );
    for (key, value, error) in [
        (KEYS[0], Value::from("false"), "InvalidArgs"),
        ("unknown", Value::from(false), "UnknownProperty"),
    ] {
        let result: zbus::Result<()> = proxy.call("Set", &(INTERFACE, key, value)).await;
        assert!(
            matches!(result, Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == format!("org.freedesktop.DBus.Error.{error}"))
        );
    }
    assert!(
        all(&proxy)
            .await
            .values()
            .all(|value| bool::try_from(value).unwrap())
    );
    policy.shutdown().await.unwrap();
    relay.await.unwrap().unwrap();
    server.close().await.unwrap();

    let mut daemon = backend(address, root.path(), false);
    ready(&proxy, &mut daemon).await;
    assert!(
        all(&proxy)
            .await
            .values()
            .all(|value| bool::try_from(value).unwrap())
    );
    set(&proxy, KEYS[1], false).await.unwrap();
    stop(&mut daemon).await;
    let mut daemon = backend(address, root.path(), true);
    ready(&proxy, &mut daemon).await;
    assert!(!get(&proxy, KEYS[1]).await);
    assert!(get(&proxy, KEYS[0]).await);
    assert!(
        matches!(set(&proxy, KEYS[0], false).await, Err(zbus::Error::MethodError(name, _, _)) if name.as_str() == "org.freedesktop.DBus.Error.AccessDenied")
    );
    assert!(get(&proxy, KEYS[0]).await);
    stop(&mut daemon).await;
}
