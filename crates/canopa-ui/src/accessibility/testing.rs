//! Native acceptance helpers: a private D-Bus and AT-SPI registry, queried
//! with plain `org.a11y.atspi` calls, as Notify's and the Shell's tests do.
pub use super::harness::Harness;
use std::time::Duration;
use zbus::zvariant::OwnedObjectPath;

const FIXTURE: &str = r#"
set -euo pipefail
fixture=$CANOPA_ACCESSIBILITY_ROOT
export XDG_RUNTIME_DIR="$fixture/runtime" XDG_CONFIG_HOME="$fixture/config"
export DCONF_BACKEND=memory GSETTINGS_BACKEND=memory ATSPI_DBUS_IMPLEMENTATION=dbus-daemon GIO_USE_VFS=local CANOPA_ACCESSIBILITY_FIXTURE=1
export DBUS_SYSTEM_BUS_ADDRESS="unix:path=$fixture/no-system-bus"
unset AT_SPI_BUS_ADDRESS NO_AT_BRIDGE DISPLAY WAYLAND_DISPLAY
mkdir -m 700 "$XDG_RUNTIME_DIR" "$XDG_CONFIG_HOME"
launcher= registry=
cleanup() {
    if [[ -n "$registry" ]]; then kill "$registry" 2>/dev/null || true; wait "$registry" 2>/dev/null || true; fi
    if [[ -n "$launcher" ]]; then kill "$launcher" 2>/dev/null || true; wait "$launcher" 2>/dev/null || true; fi
}
trap cleanup EXIT
launcher_bin=/usr/lib/at-spi-bus-launcher
registry_bin=/usr/lib/at-spi2-registryd
if [[ ! -x "$launcher_bin" ]]; then launcher_bin=/usr/libexec/at-spi-bus-launcher; fi
if [[ ! -x "$registry_bin" ]]; then registry_bin=/usr/libexec/at-spi2-registryd; fi
"$launcher_bin" --launch-immediately &
launcher=$!
for attempt in {1..100}; do
    if dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus.GetNameOwner string:org.a11y.Bus >/dev/null 2>&1; then break; fi
    sleep 0.02
done
kill -0 "$launcher"
dbus-send --session --print-reply --dest=org.a11y.Bus /org/a11y/bus org.freedesktop.DBus.Properties.Set string:org.a11y.Status string:IsEnabled variant:boolean:true >/dev/null
"$registry_bin" &
registry=$!
"$1" --exact "$2" --nocapture
"#;

/// Run the calling test again inside a private session and accessibility bus.
/// Returns `true` inside that session: the caller then runs its body.
/// `test` is the test's full path, as `--exact` expects.
pub fn isolated(test: &str) -> bool {
    if std::env::var_os("CANOPA_ACCESSIBILITY_FIXTURE").is_some() {
        return true;
    }
    let fixture = tempfile::tempdir().expect("fixture directory");
    let status = std::process::Command::new("timeout")
        .args(["60", "dbus-run-session"])
        .env("CANOPA_ACCESSIBILITY_ROOT", fixture.path())
        .args(["--", "bash", "-c", FIXTURE, "--"])
        .arg(std::env::current_exe().expect("test executable"))
        .arg(test)
        .status()
        .expect("dbus-run-session");
    assert!(status.success(), "isolated AT-SPI test failed");
    false
}

/// Connect to the private accessibility bus once its registry is up.
pub async fn connect() -> zbus::Connection {
    let session = zbus::Connection::session().await.unwrap();
    // AT-SPI 2.62 removed --a11y; enable only this private bus through its property.
    zbus::Proxy::new(&session, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Status")
        .await
        .unwrap()
        .set_property("IsEnabled", true)
        .await
        .unwrap();
    let bus = zbus::Proxy::new(&session, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus")
        .await
        .unwrap();
    let address: String = bus.call("GetAddress", &()).await.unwrap();
    let connection = zbus::connection::Builder::address(address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap();
    let names = zbus::fdo::DBusProxy::new(&connection).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !names
            .name_has_owner(zbus::names::BusName::try_from("org.a11y.atspi.Registry").unwrap())
            .await
            .unwrap()
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("private AT-SPI registry did not acquire its name");
    connection
}

/// One object on the accessibility bus.
#[derive(Clone, Debug)]
pub struct Accessible {
    connection: zbus::Connection,
    pub bus: String,
    pub path: OwnedObjectPath,
}

impl Accessible {
    async fn proxy(&self, interface: &'static str) -> zbus::Proxy<'static> {
        zbus::Proxy::new(
            &self.connection,
            self.bus.clone(),
            self.path.clone(),
            interface,
        )
        .await
        .unwrap()
    }
    pub async fn name(&self) -> String {
        self.proxy("org.a11y.atspi.Accessible")
            .await
            .get_property("Name")
            .await
            .unwrap_or_default()
    }
    pub async fn description(&self) -> String {
        self.proxy("org.a11y.atspi.Accessible")
            .await
            .get_property("Description")
            .await
            .unwrap_or_default()
    }
    /// The AT-SPI role number (`Atspi.Role`).
    pub async fn role(&self) -> u32 {
        self.proxy("org.a11y.atspi.Accessible")
            .await
            .call("GetRole", &())
            .await
            .unwrap()
    }
    /// The AT-SPI role name, such as "push button" or "list item".
    pub async fn role_name(&self) -> String {
        self.proxy("org.a11y.atspi.Accessible")
            .await
            .call("GetRoleName", &())
            .await
            .unwrap()
    }
    /// Whether `state` (an `Atspi.StateType` number) is set.
    pub async fn has_state(&self, state: u32) -> bool {
        let states: Vec<u32> = self
            .proxy("org.a11y.atspi.Accessible")
            .await
            .call("GetState", &())
            .await
            .unwrap();
        states
            .get(state as usize / 32)
            .is_some_and(|bits| bits & (1 << (state % 32)) != 0)
    }
    /// Wait until `state` is set, as focus and selection follow asynchronously.
    pub async fn wait_for_state(&self, state: u32) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while !self.has_state(state).await {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("state {state} never set"));
    }
    pub async fn do_action(&self, index: i32) -> bool {
        self.proxy("org.a11y.atspi.Action")
            .await
            .call("DoAction", &(index,))
            .await
            .unwrap()
    }
    pub async fn action_names(&self) -> Vec<String> {
        let action = self.proxy("org.a11y.atspi.Action").await;
        let count: i32 = action.get_property("NActions").await.unwrap_or(0);
        let mut names = Vec::new();
        for index in 0..count {
            names.push(action.call("GetName", &(index,)).await.unwrap());
        }
        names
    }
    pub async fn grab_focus(&self) -> bool {
        self.proxy("org.a11y.atspi.Component")
            .await
            .call("GrabFocus", &())
            .await
            .unwrap()
    }
    /// Window-relative extents: x, y, width, height.
    pub async fn extents(&self) -> (i32, i32, i32, i32) {
        let (extents,): ((i32, i32, i32, i32),) = self
            .proxy("org.a11y.atspi.Component")
            .await
            .call("GetExtents", &(1u32,))
            .await
            .unwrap();
        extents
    }
    pub async fn value(&self) -> f64 {
        self.proxy("org.a11y.atspi.Value")
            .await
            .get_property("CurrentValue")
            .await
            .unwrap()
    }
    pub async fn set_value(&self, value: f64) {
        self.proxy("org.a11y.atspi.Value")
            .await
            .set_property("CurrentValue", value)
            .await
            .unwrap();
    }
    pub async fn children(&self) -> Vec<Accessible> {
        let children: Vec<(String, OwnedObjectPath)> = self
            .proxy("org.a11y.atspi.Accessible")
            .await
            .call("GetChildren", &())
            .await
            .unwrap_or_default();
        children
            .into_iter()
            .map(|(bus, path)| Accessible {
                connection: self.connection.clone(),
                bus,
                path,
            })
            .collect()
    }
    /// The objects this one has a relation of `kind` (`Atspi.RelationType`) to.
    pub async fn relation(&self, kind: u32) -> Vec<Accessible> {
        let relations: Vec<(u32, Vec<(String, OwnedObjectPath)>)> = self
            .proxy("org.a11y.atspi.Accessible")
            .await
            .call("GetRelationSet", &())
            .await
            .unwrap_or_default();
        relations
            .into_iter()
            .filter(|(relation, _)| *relation == kind)
            .flat_map(|(_, targets)| targets)
            .map(|(bus, path)| Accessible {
                connection: self.connection.clone(),
                bus,
                path,
            })
            .collect()
    }
}

/// Search the whole registry for an object named `name`.
pub async fn find(connection: &zbus::Connection, name: &str) -> Option<Accessible> {
    let mut pending = vec![Accessible {
        connection: connection.clone(),
        bus: "org.a11y.atspi.Registry".into(),
        path: OwnedObjectPath::try_from("/org/a11y/atspi/accessible/root").unwrap(),
    }];
    for _ in 0..4096 {
        let accessible = pending.pop()?;
        if accessible.name().await == name {
            return Some(accessible);
        }
        pending.extend(accessible.children().await);
    }
    None
}

/// Wait up to five seconds for an object named `name`.
pub async fn named(connection: &zbus::Connection, name: &str) -> Accessible {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(found) = find(connection, name).await {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("AT-SPI never exposed {name}"))
}

/// Wait until no object is named `name`.
pub async fn gone(connection: &zbus::Connection, name: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while find(connection, name).await.is_some() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("AT-SPI still exposes {name}"));
}

/// AT-SPI numbers used by the acceptance tests.
pub mod atspi {
    pub const STATE_CHECKED: u32 = 4;
    pub const STATE_ENABLED: u32 = 8;
    pub const STATE_FOCUSABLE: u32 = 11;
    pub const STATE_FOCUSED: u32 = 12;
    pub const STATE_SELECTED: u32 = 23;
    pub const ROLE_CHECK_BOX: u32 = 7;
    pub const ROLE_DIALOG: u32 = 16;
    pub const ROLE_LIST: u32 = 31;
    pub const ROLE_LIST_ITEM: u32 = 32;
    pub const ROLE_PUSH_BUTTON: u32 = 43;
    pub const ROLE_SLIDER: u32 = 51;
    pub const ROLE_TEXT: u32 = 61;
    pub const ROLE_ENTRY: u32 = 79;
    pub const RELATION_LABELLED_BY: u32 = 2;
    pub const RELATION_CONTROLLER_FOR: u32 = 3;
}
