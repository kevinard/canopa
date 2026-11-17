use super::*;
use crate::provider_io::Input;
use anyhow::Result;
use canopa_panel_protocol::{PanelAction, PanelSlider};
use canopa_services::{
    backlight::{self, Backlight},
    logind,
    power_profiles::PowerProfiles,
};
use std::{path::Path, time::Duration};
use zbus::Connection;

fn profile_label(profile: &str) -> String {
    match profile {
        "power-saver" => fl!("shell-power-saver"),
        "balanced" => fl!("shell-power-balanced"),
        "performance" => fl!("shell-power-performance"),
        other => other.to_owned(),
    }
}

#[derive(Default, PartialEq, Eq)]
struct Snapshot {
    light: Option<Backlight>,
    profiles: Option<PowerProfiles>,
}

/// Whether this provider runs in a logind session, which brightness changes
/// need. Looked up until found, then kept: the session does not change.
#[derive(Default)]
struct SessionLookup(bool);

impl SessionLookup {
    async fn found(&mut self, connection: &Connection) -> bool {
        if !self.0 {
            self.0 = matches!(
                tokio::time::timeout(
                    Duration::from_millis(800),
                    logind::Session::find(connection)
                )
                .await,
                Ok(Ok(_))
            );
        }
        self.0
    }
}

impl Snapshot {
    async fn read(connection: Option<&Connection>, session: &mut SessionLookup) -> Self {
        let light = Backlight::discover(Path::new(backlight::ROOT));
        let Some(connection) = connection else {
            return Self::default();
        };
        let (profiles, session) = tokio::join!(
            PowerProfiles::discover(connection),
            session.found(connection)
        );
        Self {
            light: light.filter(|_| session),
            profiles,
        }
    }

    fn content(&self, error: Option<&str>) -> PanelContent {
        let mut content = PanelContent::new(fl!("shell-power-title"));
        content.error = error.map(str::to_owned);
        let light = if let Some(light) = &self.light {
            let mut row = item("display", fl!("shell-power-built-in-display"));
            row.slider = Some(PanelSlider {
                key: "brightness".into(),
                label: fl!("shell-osd-brightness"),
                min: 1,
                max: 100,
                step: 1,
                // The slider moves on the same perceptual curve as the brightness keys.
                value: light.perceptual().max(1),
                unit: "%".into(),
            });
            row
        } else {
            item("display", fl!("shell-power-brightness-unavailable"))
        };
        let mut profiles = item("profiles", fl!("shell-power-profile"));
        if let Some(state) = &self.profiles {
            profiles.body = Some(fl!(
                "shell-power-profile-active",
                profile = profile_label(&state.active)
            ));
            profiles.actions = state
                .supported
                .iter()
                .filter(|name| **name != state.active)
                .map(|name| PanelAction::new(*name, profile_label(name)))
                .collect();
        } else {
            profiles.body = Some(fl!("shell-power-profiles-unavailable"));
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
    let mut session = SessionLookup::default();
    let mut connection = None;
    let mut error = None;
    let mut tick = tokio::time::interval(Duration::from_secs(3));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // The first frame waits for the first read, so a present service never
    // flashes "unavailable"; a slow or absent D-Bus still gets a frame quickly.
    let mut snapshot = tokio::time::timeout(FIRST_READ, async {
        connection = tokio::time::timeout(Duration::from_secs(1), Connection::system())
            .await
            .ok()
            .and_then(Result::ok);
        Snapshot::read(connection.as_ref(), &mut session).await
    })
    .await
    .unwrap_or_default();
    publish(&mut panel, output, snapshot.content(None)).await?;
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if connection.is_none() {
                    connection = tokio::time::timeout(Duration::from_secs(1), Connection::system())
                        .await.ok().and_then(Result::ok);
                }
                let next = Snapshot::read(connection.as_ref(), &mut session).await;
                if next != snapshot {
                    snapshot = next;
                    publish(&mut panel, output, snapshot.content(error.as_deref())).await?;
                }
            }
            message = input.next() => {
                let Some(message) = message? else { return Ok(()); };
                if matches!(message, HostMessage::Close { .. }) { return Ok(()); }
                let (Some(connection), Some((id, key, value))) = (connection.as_ref(), action(&panel, &message)) else { continue; };
                let result = match (id, key, value) {
                    ("display", "brightness", Some(value)) if let Some(light) = &snapshot.light => {
                        let Ok(percent) = value.parse::<u16>() else { continue; };
                        tokio::time::timeout(Duration::from_secs(2), light.set_perceptual(connection, percent)).await
                    }
                    ("profiles", profile, None) if let Some(profiles) = &snapshot.profiles && profiles.supported.contains(&profile) => {
                        tokio::time::timeout(Duration::from_secs(2), profiles.set(connection, profile)).await
                    }
                    _ => continue,
                };
                error = (!matches!(result, Ok(Ok(())))).then(|| fl!("shell-power-change-failed"));
                snapshot = Snapshot::read(Some(connection), &mut session).await;
                publish(&mut panel, output, snapshot.content(error.as_deref())).await?;
            }
        }
    }
}
