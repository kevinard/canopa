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

fn profile_label(profile: &str) -> &str {
    match profile {
        "power-saver" => "Power saver",
        "balanced" => "Balanced",
        "performance" => "Performance",
        other => other,
    }
}

#[derive(Default, PartialEq, Eq)]
struct Snapshot {
    light: Option<Backlight>,
    profiles: Option<PowerProfiles>,
}

impl Snapshot {
    async fn read(connection: Option<&Connection>) -> Self {
        let light = Backlight::discover(Path::new(backlight::ROOT));
        let Some(connection) = connection else {
            return Self::default();
        };
        let (profiles, session) = tokio::join!(
            PowerProfiles::discover(connection),
            tokio::time::timeout(
                Duration::from_millis(800),
                logind::Session::find(connection)
            ),
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
