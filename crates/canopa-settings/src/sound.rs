//! The Sound page's models: devices, application streams and card profiles
//! from the audio server (PipeWire through its PulseAudio protocol), and the
//! notification sound switch in Notify's configuration.
use crate::{
    live::{LiveAction, LiveService, ServiceError},
    session::Paths,
    storage::Snapshot,
};
use anyhow::{Context, Result};
use canopa_services::audio::{
    self, AudioControl, AudioDeviceId, AudioError, AudioSnapshot, AudioStreamId, DeviceOperation,
    StreamOperation,
};
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::sync::watch;

/// A change the Sound page asks the audio server for.
#[derive(Clone, Debug, PartialEq)]
pub enum SoundAction {
    Volume(AudioDeviceId, u8),
    Mute(AudioDeviceId, bool),
    Default(AudioDeviceId),
    Balance(AudioDeviceId, i8),
    StreamVolume(AudioStreamId, u8),
    StreamMute(AudioStreamId, bool),
    /// A card (index, name) and the profile to use.
    Profile(u32, String, String),
}

fn device_key(id: &AudioDeviceId) -> String {
    format!("{:?}-{}", id.kind, id.name).to_lowercase()
}

fn stream_key(id: &AudioStreamId) -> String {
    format!("{:?}-{}", id.kind, id.index).to_lowercase()
}

impl LiveAction for SoundAction {
    fn control(&self) -> String {
        match self {
            Self::Volume(id, _) => format!("volume-{}", device_key(id)),
            Self::Mute(id, _) => format!("mute-{}", device_key(id)),
            Self::Balance(id, _) => format!("balance-{}", device_key(id)),
            // One default per direction.
            Self::Default(id) => format!("default-{:?}", id.kind).to_lowercase(),
            Self::StreamVolume(id, _) => format!("stream-volume-{}", stream_key(id)),
            Self::StreamMute(id, _) => format!("stream-mute-{}", stream_key(id)),
            Self::Profile(_, name, _) => format!("profile-{name}"),
        }
    }
}

impl SoundAction {
    /// The control name a view uses for this kind of change on a device.
    pub fn device_control(kind: &str, id: &AudioDeviceId) -> String {
        format!("{kind}-{}", device_key(id))
    }
    pub fn stream_control(kind: &str, id: &AudioStreamId) -> String {
        format!("stream-{kind}-{}", stream_key(id))
    }
}

const NO_SERVER: &str = "No audio server answers (PipeWire or PulseAudio).";

struct Mixer {
    control: AudioControl,
    state: watch::Receiver<Option<AudioSnapshot>>,
}

/// The audio server's mixer for the page. The connection opens on first use
/// and lasts while the window runs; it records nothing.
#[derive(Clone, Default)]
pub struct SoundService {
    server: Option<String>,
    mixer: Arc<OnceLock<Mixer>>,
}

impl SoundService {
    /// A service on a specific Pulse server address (tests).
    pub fn with_server(server: String) -> Self {
        Self {
            server: Some(server),
            mixer: Arc::default(),
        }
    }

    /// The Pulse server this page uses; `None` is the default server.
    pub fn server(&self) -> Option<String> {
        self.server.clone()
    }

    fn mixer(&self) -> &Mixer {
        self.mixer.get_or_init(|| {
            audio::set_client_name("Canopa Settings");
            let (control, state) = audio::mixer(self.server.clone());
            Mixer { control, state }
        })
    }

    async fn current(
        mut state: watch::Receiver<Option<AudioSnapshot>>,
    ) -> Result<AudioSnapshot, ServiceError> {
        let wait = state.wait_for(Option::is_some);
        match tokio::time::timeout(Duration::from_secs(3), wait).await {
            Ok(Ok(snapshot)) => Ok(snapshot.clone().expect("waited for a snapshot")),
            _ => Err(ServiceError::Unavailable(NO_SERVER.into())),
        }
    }
}

fn failure(error: AudioError) -> ServiceError {
    match error {
        AudioError::Unavailable | AudioError::ControlsUnavailable => {
            ServiceError::Unavailable(NO_SERVER.into())
        }
        other => ServiceError::Failed(other.to_string()),
    }
}

impl LiveService for SoundService {
    type State = AudioSnapshot;
    type Action = SoundAction;

    fn name(&self) -> &str {
        "The audio server"
    }

    fn snapshot(&self) -> BoxFuture<'static, Result<AudioSnapshot, ServiceError>> {
        Self::current(self.mixer().state.clone()).boxed()
    }

    fn updates(&self) -> BoxStream<'static, Result<AudioSnapshot, ServiceError>> {
        let mut state = self.mixer().state.clone();
        state.mark_unchanged();
        futures_util::stream::unfold(Some(state), |state| async move {
            let mut state = state?;
            state.changed().await.ok()?;
            let current = state.borrow_and_update().clone();
            Some(match current {
                Some(snapshot) => (Ok(snapshot), Some(state)),
                // The monitor reconnects; the page waits and reads again.
                None => (Err(ServiceError::Unavailable(NO_SERVER.into())), None),
            })
        })
        .boxed()
    }

    fn apply(
        &self,
        action: SoundAction,
    ) -> BoxFuture<'static, Result<Option<AudioSnapshot>, ServiceError>> {
        let mixer = self.mixer();
        let control = mixer.control.clone();
        let state = mixer.state.clone();
        async move {
            let result = match action {
                SoundAction::Volume(id, percent) => {
                    control
                        .apply(id, DeviceOperation::SetVolume { percent })
                        .await
                }
                SoundAction::Mute(id, muted) => {
                    control.apply(id, DeviceOperation::SetMuted(muted)).await
                }
                SoundAction::Default(id) => control.apply(id, DeviceOperation::SetDefault).await,
                SoundAction::Balance(id, balance) => {
                    control
                        .apply(id, DeviceOperation::SetBalance { balance })
                        .await
                }
                SoundAction::StreamVolume(id, percent) => {
                    control
                        .apply_stream(id, StreamOperation::SetVolume { percent })
                        .await
                }
                SoundAction::StreamMute(id, muted) => {
                    control
                        .apply_stream(id, StreamOperation::SetMuted(muted))
                        .await
                }
                SoundAction::Profile(index, name, profile) => {
                    let snapshot = Self::current(state.clone()).await?;
                    let card = snapshot
                        .cards
                        .iter()
                        .find(|card| card.index == index && card.name == name)
                        .ok_or_else(|| failure(AudioError::DeviceGone))?;
                    control.set_profile(card, &profile).await
                }
            };
            result.map_err(failure)?;
            // The monitor publishes the change; show the latest reading now.
            Ok(state.borrow().clone())
        }
        .boxed()
    }
}

/// Whether Notify plays notification sounds (`notifications { sound
/// enabled=… }` in `config.kdl`; on by default).
pub fn alert_sounds(text: &str) -> Result<bool> {
    if text.trim().is_empty() {
        return Ok(true);
    }
    let document: kdl::KdlDocument = text.parse()?;
    Ok(document
        .get("notifications")
        .and_then(kdl::KdlNode::children)
        .and_then(|children| children.get("sound"))
        .and_then(|sound| sound.get("enabled"))
        .and_then(kdl::KdlValue::as_bool)
        .unwrap_or(true))
}

/// `text` with the notification sound switch set, keeping everything else.
pub fn with_alert_sounds(text: &str, enabled: bool) -> Result<String> {
    let mut document: kdl::KdlDocument = if text.trim().is_empty() {
        "schema-version 1\n".parse()?
    } else {
        text.parse()?
    };
    let notifications = crate::preferences::block(&mut document, "notifications")?;
    if notifications.get("sound").is_none() {
        notifications.nodes_mut().push(kdl::KdlNode::new("sound"));
    }
    let sound = notifications
        .get_mut("sound")
        .context("missing sound node")?;
    sound.insert("enabled", enabled);
    Ok(document.to_string())
}

/// Turn notification sounds on or off: validate and save `config.kdl`, then
/// ask Notify to reload. Returns the replaced and the saved versions, for the
/// pages that share the file.
pub fn set_alert_sounds(paths: &Paths, enabled: bool) -> Result<(Snapshot, Snapshot)> {
    let source = Snapshot::load(paths.common())?;
    let text = with_alert_sounds(&source.text, enabled)?;
    let saved = crate::preferences::save_common(&source, &text)?;
    let _ = crate::storage::command(
        crate::storage::companion("canopa-notify"),
        &["ctl".into(), "reload".into()],
    );
    Ok((source, saved))
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopa_services::{audio::DeviceKind, testing::audio::AudioServer};

    #[test]
    fn the_notification_sound_switch_keeps_the_rest_of_the_file() {
        assert!(alert_sounds("").unwrap());
        let text = "schema-version 1\n// keep\nnotifications {\n    sound theme=\"freedesktop\"\n    rule \"x\" app=\"a\" outcome=\"silenced\"\n}\n";
        assert!(alert_sounds(text).unwrap());
        let off = with_alert_sounds(text, false).unwrap();
        assert!(!alert_sounds(&off).unwrap());
        assert!(
            off.contains("// keep")
                && off.contains("theme=\"freedesktop\"")
                && off.contains("rule \"x\"")
        );
        assert!(alert_sounds(&with_alert_sounds(&off, true).unwrap()).unwrap());
        let created = with_alert_sounds("schema-version 1\n", false).unwrap();
        assert!(!alert_sounds(&created).unwrap());
        // Personalization and Widgets keep their sections, so they rebase.
        assert_eq!(
            crate::preferences::common_sections(text).unwrap(),
            crate::preferences::common_sections(&off).unwrap()
        );
    }

    #[tokio::test]
    async fn the_page_follows_the_mixer_and_is_unavailable_without_a_server() {
        let missing = SoundService::with_server("unix:/nonexistent/pulse".into());
        assert!(matches!(
            missing.snapshot().await,
            Err(ServiceError::Unavailable(_))
        ));

        let server = AudioServer::new().await;
        server
            .pactl(&["load-module", "module-null-sink", "sink_name=speakers"])
            .await;
        server
            .pactl(&["load-module", "module-null-sink", "sink_name=headphones"])
            .await;
        server.set_default_sink("speakers").await;
        let service = SoundService::with_server(server.server.clone());
        let snapshot = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(snapshot) = service.snapshot().await
                    && snapshot.devices.len() == 2
                {
                    return snapshot;
                }
            }
        })
        .await
        .unwrap();
        let headphones = snapshot
            .devices
            .iter()
            .find(|device| device.id.name == "headphones" && device.id.kind == DeviceKind::Output)
            .unwrap()
            .id
            .clone();
        let mut updates = service.updates();
        for action in [
            SoundAction::Volume(headphones.clone(), 30),
            SoundAction::Mute(headphones.clone(), true),
            SoundAction::Balance(headphones.clone(), -40),
            SoundAction::Default(headphones.clone()),
        ] {
            service.apply(action).await.unwrap();
        }
        let volume = server.pactl(&["get-sink-volume", "headphones"]).await;
        assert!(volume.contains("30%"), "{volume}");
        assert!(
            server
                .pactl(&["get-sink-mute", "headphones"])
                .await
                .contains("yes")
        );
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(3), updates.next()).await,
            Ok(Some(Ok(_)))
        ));
        // A card that is gone is a refused change, not an outage.
        assert!(matches!(
            service
                .apply(SoundAction::Profile(99, "gone".into(), "off".into()))
                .await,
            Err(ServiceError::Failed(_))
        ));
        assert_ne!(
            SoundAction::Volume(headphones.clone(), 1).control(),
            SoundAction::Mute(headphones.clone(), true).control()
        );
        assert_eq!(
            SoundAction::Volume(headphones.clone(), 1).control(),
            SoundAction::device_control("volume", &headphones)
        );
    }
}
