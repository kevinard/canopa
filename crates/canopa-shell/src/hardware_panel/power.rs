use super::*;
use crate::provider_io::Input;
use anyhow::{Context, Result, ensure};
use canopa_panel_protocol::{PanelAction, PanelSlider};
use std::{collections::HashMap, path::Path, time::Duration};
use zbus::{
    Connection, Proxy,
    zvariant::{OwnedObjectPath, OwnedValue},
};

const BACKLIGHT: &str = "/sys/class/backlight";
const LOGIN: &str = "org.freedesktop.login1";
const PROFILE_SERVICES: [(&str, &str); 2] = [
    (
        "org.freedesktop.UPower.PowerProfiles",
        "/org/freedesktop/UPower/PowerProfiles",
    ),
    ("net.hadess.PowerProfiles", "/net/hadess/PowerProfiles"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct Backlight {
    name: String,
    max: u32,
    value: u32,
}

impl Backlight {
    fn discover(root: &Path) -> Option<Self> {
        std::fs::read_dir(root)
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                let priority = match std::fs::read_to_string(path.join("type")).ok()?.trim() {
                    "firmware" => 0,
                    "platform" => 1,
                    "raw" => 2,
                    _ => return None,
                };
                let max: u32 = std::fs::read_to_string(path.join("max_brightness"))
                    .ok()?
                    .trim()
                    .parse()
                    .ok()?;
                let value: u32 = std::fs::read_to_string(path.join("brightness"))
                    .ok()?
                    .trim()
                    .parse()
                    .ok()?;
                (max > 0 && value <= max).then_some((
                    priority,
                    Self {
                        name: entry.file_name().into_string().ok()?,
                        max,
                        value,
                    },
                ))
            })
            .min_by(|a, b| (a.0, &a.1.name).cmp(&(b.0, &b.1.name)))
            .map(|(_, light)| light)
    }

    fn percent(&self) -> u16 {
        ((u64::from(self.value) * 100 + u64::from(self.max) / 2) / u64::from(self.max)) as u16
    }

    async fn set(&self, connection: &Connection, percent: u16) -> Result<()> {
        ensure!(
            (1..=100).contains(&percent),
            "Brightness must be between 1% and 100%."
        );
        let current =
            Self::discover(Path::new(BACKLIGHT)).context("Backlight was disconnected.")?;
        ensure!(
            self.name == current.name && self.max == current.max,
            "Backlight changed."
        );
        let session = session(connection).await?;
        // Round to the nearest hardware step and keep the panel lit.
        let level = ((u64::from(self.max) * u64::from(percent) + 50) / 100).max(1) as u32;
        session
            .call::<_, _, ()>("SetBrightness", &("backlight", &self.name, level))
            .await?;
        Ok(())
    }
}

async fn session(connection: &Connection) -> Result<Proxy<'_>> {
    let launcher = std::env::var("CANOPA_SESSION_PID")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|pid| *pid != 0);
    let session_id = std::env::var("XDG_SESSION_ID").ok();
    let path = if launcher.is_some() || session_id.is_some() {
        let manager = Proxy::new(
            connection,
            LOGIN,
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )
        .await?;
        if let Some(pid) = launcher {
            manager.call("GetSessionByPID", &(pid,)).await?
        } else {
            manager.call("GetSession", &(session_id.unwrap(),)).await?
        }
    } else {
        OwnedObjectPath::try_from("/org/freedesktop/login1/session/auto")?
    };
    let proxy = Proxy::new(connection, LOGIN, path, "org.freedesktop.login1.Session").await?;
    ensure!(
        proxy.get_property::<bool>("Active").await?
            && !proxy.get_property::<bool>("Remote").await?,
        "An active local login is required."
    );
    ensure!(
        proxy.get_property::<String>("Type").await? == "wayland",
        "A Wayland login is required."
    );
    Ok(proxy)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Profiles {
    service: usize,
    active: String,
    supported: Vec<&'static str>,
}

fn profile_label(profile: &str) -> &str {
    match profile {
        "power-saver" => "Power saver",
        "balanced" => "Balanced",
        "performance" => "Performance",
        other => other,
    }
}

impl Profiles {
    async fn proxy(connection: &Connection, service: usize) -> zbus::Result<Proxy<'_>> {
        let (name, path) = PROFILE_SERVICES[service];
        Proxy::new(connection, name, path, name).await
    }

    async fn read(connection: &Connection, service: usize) -> zbus::Result<Self> {
        let proxy = Self::proxy(connection, service).await?;
        let (active, profiles) = tokio::try_join!(
            proxy.get_property("ActiveProfile"),
            proxy.get_property::<Vec<HashMap<String, OwnedValue>>>("Profiles"),
        )?;
        let supported = ["power-saver", "balanced", "performance"]
            .into_iter()
            .filter(|name| {
                profiles.iter().any(|profile| {
                    profile
                        .get("Profile")
                        .and_then(|value| <&str>::try_from(value).ok())
                        == Some(*name)
                })
            })
            .collect();
        Ok(Self {
            service,
            active,
            supported,
        })
    }

    async fn discover(connection: &Connection) -> Option<Self> {
        for service in 0..PROFILE_SERVICES.len() {
            if let Ok(Ok(profiles)) =
                tokio::time::timeout(Duration::from_millis(800), Self::read(connection, service))
                    .await
            {
                return Some(profiles);
            }
        }
        None
    }

    async fn set(&self, connection: &Connection, profile: &str) -> Result<()> {
        let current = Self::read(connection, self.service).await?;
        ensure!(
            current.supported.contains(&profile),
            "Power profile is no longer available."
        );
        Self::proxy(connection, self.service)
            .await?
            .set_property("ActiveProfile", profile)
            .await?;
        Ok(())
    }
}

#[derive(Default, PartialEq, Eq)]
struct Snapshot {
    light: Option<Backlight>,
    profiles: Option<Profiles>,
}

impl Snapshot {
    async fn read(connection: Option<&Connection>) -> Self {
        let light = Backlight::discover(Path::new(BACKLIGHT));
        let Some(connection) = connection else {
            return Self::default();
        };
        let (profiles, session) = tokio::join!(
            Profiles::discover(connection),
            tokio::time::timeout(Duration::from_millis(800), session(connection)),
        );
        Self {
            light: light.filter(|_| matches!(session, Ok(Ok(_)))),
            profiles,
        }
    }

    fn content(&self, error: Option<&str>) -> PanelContent {
        let mut content = PanelContent::new("Power & brightness");
        content.error = error.map(str::to_owned);
        let light = if let Some(light) = &self.light {
            let mut row = item("display", "Built-in display");
            row.slider = Some(PanelSlider {
                key: "brightness".into(),
                label: "Brightness".into(),
                min: 1,
                max: 100,
                step: 1,
                value: light.percent().max(1),
                unit: "%".into(),
            });
            row
        } else {
            item(
                "display",
                "Display brightness is unavailable in this session.",
            )
        };
        let mut profiles = item("profiles", "Power profile");
        if let Some(state) = &self.profiles {
            profiles.body = Some(format!("Active: {}", profile_label(&state.active)));
            profiles.actions = state
                .supported
                .iter()
                .filter(|name| **name != state.active)
                .map(|name| PanelAction::new(*name, profile_label(name)))
                .collect();
        } else {
            profiles.body = Some("Power profiles service is unavailable.".into());
        }
        content.sections = vec![
            section("brightness", "", vec![light]),
            section("power", "", vec![profiles]),
        ];
        content
    }
}

pub(super) async fn run(
    mut input: Input<HostMessage>,
    output: &mut (impl AsyncWrite + Unpin),
) -> Result<()> {
    let mut panel = Session::default();
    let mut snapshot = Snapshot::default();
    let mut connection = None;
    let mut error = None;
    let mut tick = tokio::time::interval(Duration::from_secs(3));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // Send a frame immediately even when D-Bus is slow or absent.
    publish(&mut panel, output, snapshot.content(None)).await?;
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if connection.is_none() {
                    connection = tokio::time::timeout(Duration::from_secs(1), Connection::system())
                        .await.ok().and_then(Result::ok);
                }
                let next = Snapshot::read(connection.as_ref()).await;
                if next != snapshot {
                    snapshot = next;
                    publish(&mut panel, output, snapshot.content(error)).await?;
                }
            }
            message = input.next() => {
                let Some(message) = message? else { return Ok(()); };
                if matches!(message, HostMessage::Close { .. }) { return Ok(()); }
                let (Some(connection), Some((id, key, value))) = (connection.as_ref(), action(&panel, &message)) else { continue; };
                let result = match (id, key, value) {
                    ("display", "brightness", Some(value)) if let Some(light) = &snapshot.light => {
                        let Ok(percent) = value.parse::<u16>() else { continue; };
                        tokio::time::timeout(Duration::from_secs(2), light.set(connection, percent)).await
                    }
                    ("profiles", profile, None) if let Some(profiles) = &snapshot.profiles && profiles.supported.contains(&profile) => {
                        tokio::time::timeout(Duration::from_secs(2), profiles.set(connection, profile)).await
                    }
                    _ => continue,
                };
                error = (!matches!(result, Ok(Ok(())))).then_some("The system could not apply the change.");
                snapshot = Snapshot::read(Some(connection)).await;
                publish(&mut panel, output, snapshot.content(error)).await?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_firmware_backlight_and_uses_hardware_range() {
        let root = tempfile::tempdir().unwrap();
        for (name, kind, max, brightness) in [
            ("intel", "raw", "100", "75"),
            ("acpi", "firmware", "400", "100"),
            ("bad", "firmware", "0", "0"),
        ] {
            let path = root.path().join(name);
            std::fs::create_dir(&path).unwrap();
            std::fs::write(path.join("type"), kind).unwrap();
            std::fs::write(path.join("max_brightness"), max).unwrap();
            std::fs::write(path.join("brightness"), brightness).unwrap();
        }
        let light = Backlight::discover(root.path()).unwrap();
        assert_eq!(light.name, "acpi");
        assert_eq!(light.percent(), 25);
    }
}
