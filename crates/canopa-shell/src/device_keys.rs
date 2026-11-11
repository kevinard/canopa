//! Device keys: volume, microphone, display and keyboard brightness, media.
//!
//! The keys run `canopa-shell volume up` and similar commands. The running
//! Shell executes the [`Request`] and shows the resulting [`Indicator`] in its
//! on-screen display; with no Shell running, the command runs the same
//! [`Keys::run`] itself. Services own the levels: audio goes through the
//! Pulse-protocol server, brightness through logind `SetBrightness` (no root),
//! media through MPRIS with the Shell's selected player
//! (`org.canopa.Shell.Media`, Shell ADR 0018). A missing service is an
//! ordinary [`Outcome::Unavailable`], never an error to log.
use canopa_services::{
    audio::{self, DefaultChange, DeviceKind},
    backlight::{self, Backlight, KeyboardBacklight},
    mpris::{self, Operation},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, time::Duration};

/// One device key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    VolumeUp,
    VolumeDown,
    VolumeMute,
    MicrophoneMute,
    BrightnessUp,
    BrightnessDown,
    KeyboardBacklightUp,
    KeyboardBacklightDown,
    KeyboardBacklightToggle,
    MediaPlayPause,
    MediaPlay,
    MediaPause,
    MediaStop,
    MediaNext,
    MediaPrevious,
}

/// A key press, with an optional step that overrides the configured one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub action: Action,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<u8>,
}

/// The CLI words for each action, in usage order.
pub const COMMANDS: &[(&str, &[(&str, Action)])] = &[
    (
        "volume",
        &[
            ("up", Action::VolumeUp),
            ("down", Action::VolumeDown),
            ("mute", Action::VolumeMute),
        ],
    ),
    ("microphone", &[("mute", Action::MicrophoneMute)]),
    (
        "brightness",
        &[
            ("up", Action::BrightnessUp),
            ("down", Action::BrightnessDown),
        ],
    ),
    (
        "keyboard-backlight",
        &[
            ("up", Action::KeyboardBacklightUp),
            ("down", Action::KeyboardBacklightDown),
            ("toggle", Action::KeyboardBacklightToggle),
        ],
    ),
    (
        "media",
        &[
            ("play-pause", Action::MediaPlayPause),
            ("play", Action::MediaPlay),
            ("pause", Action::MediaPause),
            ("stop", Action::MediaStop),
            ("next", Action::MediaNext),
            ("previous", Action::MediaPrevious),
        ],
    ),
];

impl Action {
    /// The largest step this action accepts, if it takes one.
    fn max_step(self) -> Option<u8> {
        match self {
            Self::VolumeUp | Self::VolumeDown => Some(100),
            Self::BrightnessUp | Self::BrightnessDown => Some(50),
            _ => None,
        }
    }

    pub fn channel(self) -> Option<Channel> {
        Some(match self {
            Self::VolumeUp | Self::VolumeDown | Self::VolumeMute => Channel::Volume,
            Self::MicrophoneMute => Channel::Microphone,
            Self::BrightnessUp | Self::BrightnessDown => Channel::Brightness,
            Self::KeyboardBacklightUp
            | Self::KeyboardBacklightDown
            | Self::KeyboardBacklightToggle => Channel::KeyboardBacklight,
            _ => return None,
        })
    }
}

/// Whether `word` starts a device-key command (`volume`, `media`, ...).
pub fn is_command(word: &str) -> bool {
    word == "mic" || COMMANDS.iter().any(|(name, _)| *name == word)
}

/// Parse `volume up [STEP]` and the other commands.
pub fn parse(args: &[String]) -> Result<Request, String> {
    let usage = || {
        COMMANDS
            .iter()
            .map(|(name, actions)| {
                let verbs = actions.iter().map(|(verb, _)| *verb).collect::<Vec<_>>();
                format!("{name} {}", verbs.join("|"))
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let [command, verb, rest @ ..] = args else {
        return Err(format!("expected one of: {}", usage()));
    };
    let command = if command == "mic" {
        "microphone"
    } else {
        command
    };
    let action = COMMANDS
        .iter()
        .find(|(name, _)| *name == command)
        .and_then(|(_, actions)| actions.iter().find(|(name, _)| name == verb))
        .map(|(_, action)| *action)
        .ok_or_else(|| format!("expected one of: {}", usage()))?;
    let step = match (rest, action.max_step()) {
        ([], _) => None,
        ([step], Some(max)) => Some(
            step.parse::<u8>()
                .ok()
                .filter(|step| (1..=max).contains(step))
                .ok_or_else(|| format!("{command} {verb} takes a step from 1 to {max}"))?,
        ),
        _ => return Err(format!("unexpected argument after {command} {verb}")),
    };
    Ok(Request { action, step })
}

/// What an indicator shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Channel {
    Volume,
    Microphone,
    Brightness,
    KeyboardBacklight,
}

impl Channel {
    pub fn title(self) -> &'static str {
        match self {
            Self::Volume => "Volume",
            Self::Microphone => "Microphone",
            Self::Brightness => "Brightness",
            Self::KeyboardBacklight => "Keyboard brightness",
        }
    }
}

/// The content of the on-screen display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Indicator {
    Level {
        channel: Channel,
        /// 0–100 for the bar; volume can exceed 100 when another program
        /// amplified it.
        percent: u16,
        /// Muted output or microphone, or a keyboard backlight that is off.
        muted: bool,
        /// The output or input device, for audio.
        device: Option<String>,
    },
    /// The service or device is missing; shown briefly.
    Unavailable(Channel),
}

impl Indicator {
    pub fn channel(&self) -> Channel {
        match self {
            Self::Level { channel, .. } | Self::Unavailable(channel) => *channel,
        }
    }

    /// The bar's fill, 0.0–1.0, if there is a level.
    pub fn fill(&self) -> Option<f32> {
        match self {
            Self::Level { percent, .. } => Some(f32::from((*percent).min(100)) / 100.0),
            Self::Unavailable(_) => None,
        }
    }

    pub fn muted(&self) -> bool {
        matches!(self, Self::Level { muted: true, .. })
    }

    /// The value beside the bar: `45%`, `Muted`, `Off` or `Unavailable`.
    pub fn value(&self) -> String {
        match self {
            Self::Level {
                channel: Channel::Volume | Channel::Microphone,
                muted: true,
                ..
            } => "Muted".into(),
            Self::Level {
                channel: Channel::KeyboardBacklight,
                muted: true,
                ..
            } => "Off".into(),
            Self::Level { percent, .. } => format!("{percent}%"),
            Self::Unavailable(_) => "Unavailable".into(),
        }
    }

    /// What a screen reader announces, for example
    /// `Volume 45%, Speakers` or `Microphone muted, Built-in microphone`.
    pub fn announcement(&self) -> String {
        let channel = self.channel().title();
        let value = match self {
            Self::Unavailable(_) => "unavailable".to_owned(),
            Self::Level { muted: true, .. } => self.value().to_lowercase(),
            Self::Level { .. } => self.value(),
        };
        match self {
            Self::Level {
                device: Some(device),
                ..
            } => format!("{channel} {value}, {device}"),
            _ => format!("{channel} {value}"),
        }
    }

    /// The parts of an indicator that make an outside change worth showing.
    /// Microphone levels are left out: voice applications adjust them all the
    /// time, while mute is a deliberate change.
    pub fn significant(&self) -> (Channel, Option<&str>, Option<u16>, bool, bool) {
        match self {
            Self::Level {
                channel,
                percent,
                muted,
                device,
            } => (
                *channel,
                device.as_deref(),
                (*channel != Channel::Microphone).then_some(*percent),
                *muted,
                true,
            ),
            Self::Unavailable(channel) => (*channel, None, None, false, false),
        }
    }
}

/// The configured step sizes, in percent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Steps {
    /// Volume step, 1–100.
    pub volume: u8,
    /// Perceptual brightness step, 1–50.
    pub brightness: u8,
}

impl Default for Steps {
    fn default() -> Self {
        Self {
            volume: 5,
            brightness: 5,
        }
    }
}

/// The result of a key press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Changed(Indicator),
    /// A media control reached the player.
    Played,
    /// The level's service or device is missing.
    Unavailable(Channel),
    /// No media player, or it refused the control.
    NoPlayer,
}

impl Outcome {
    pub fn succeeded(&self) -> bool {
        matches!(self, Self::Changed(_) | Self::Played)
    }

    /// What the on-screen display shows, if anything.
    pub fn indicator(&self) -> Option<Indicator> {
        match self {
            Self::Changed(indicator) => Some(indicator.clone()),
            Self::Unavailable(channel) => Some(Indicator::Unavailable(*channel)),
            Self::Played | Self::NoPlayer => None,
        }
    }
}

const SERVICE_DEADLINE: Duration = Duration::from_secs(3);

/// Executes key presses. One instance serializes brightness changes and
/// remembers the keyboard backlight level that toggling turns back on.
pub struct Keys {
    /// The Pulse server; `None` is the session's.
    pub audio_server: Option<String>,
    pub backlights: PathBuf,
    pub leds: PathBuf,
    system: tokio::sync::Mutex<Option<zbus::Connection>>,
    session: tokio::sync::Mutex<Option<zbus::Connection>>,
    /// Given buses are kept; connected ones are dropped after a failure, so
    /// a restarted bus daemon is reached again on the next press.
    given: bool,
    backlight: tokio::sync::Mutex<HashMap<String, u32>>,
}

impl Default for Keys {
    fn default() -> Self {
        Self::new(
            None,
            PathBuf::from(backlight::ROOT),
            PathBuf::from(backlight::LEDS),
        )
    }
}

impl Keys {
    pub fn new(audio_server: Option<String>, backlights: PathBuf, leds: PathBuf) -> Self {
        Self {
            audio_server,
            backlights,
            leds,
            system: Default::default(),
            session: Default::default(),
            given: false,
            backlight: Default::default(),
        }
    }

    /// Use these buses instead of connecting to the system and session buses.
    pub fn with_buses(
        mut self,
        system: Option<zbus::Connection>,
        session: Option<zbus::Connection>,
    ) -> Self {
        self.system = tokio::sync::Mutex::new(system);
        self.session = tokio::sync::Mutex::new(session);
        self.given = true;
        self
    }

    async fn bus(
        slot: &tokio::sync::Mutex<Option<zbus::Connection>>,
        system: bool,
    ) -> Option<zbus::Connection> {
        let mut slot = slot.lock().await;
        if slot.is_none() {
            let connect = async {
                if system {
                    canopa_services::bus::system().await
                } else {
                    canopa_services::bus::session().await
                }
            };
            *slot = tokio::time::timeout(Duration::from_secs(1), connect)
                .await
                .ok()
                .and_then(Result::ok);
        }
        slot.clone()
    }

    async fn forget(&self, slot: &tokio::sync::Mutex<Option<zbus::Connection>>) {
        if !self.given {
            *slot.lock().await = None;
        }
    }

    /// Run one key press.
    pub async fn run(&self, request: Request, steps: Steps) -> Outcome {
        use Action::*;
        let volume = request.step.unwrap_or(steps.volume).clamp(1, 100);
        let brightness = request.step.unwrap_or(steps.brightness).clamp(1, 50);
        match request.action {
            VolumeUp => {
                self.audio(DeviceKind::Output, DefaultChange::Raise(volume))
                    .await
            }
            VolumeDown => {
                self.audio(DeviceKind::Output, DefaultChange::Lower(volume))
                    .await
            }
            VolumeMute => {
                self.audio(DeviceKind::Output, DefaultChange::ToggleMute)
                    .await
            }
            MicrophoneMute => {
                self.audio(DeviceKind::Input, DefaultChange::ToggleMute)
                    .await
            }
            BrightnessUp => self.display(brightness, true).await,
            BrightnessDown => self.display(brightness, false).await,
            KeyboardBacklightUp => self.keyboard(Some(true)).await,
            KeyboardBacklightDown => self.keyboard(Some(false)).await,
            KeyboardBacklightToggle => self.keyboard(None).await,
            MediaPlayPause | MediaPlay | MediaPause | MediaStop | MediaNext | MediaPrevious => {
                self.media(request.action).await
            }
        }
    }

    async fn audio(&self, kind: DeviceKind, change: DefaultChange) -> Outcome {
        let channel = match kind {
            DeviceKind::Output => Channel::Volume,
            DeviceKind::Input => Channel::Microphone,
        };
        let result = tokio::time::timeout(
            SERVICE_DEADLINE,
            audio::change_default(self.audio_server.clone(), kind, change),
        )
        .await;
        match result {
            Ok(Ok(device)) => Outcome::Changed(audio_indicator(channel, &device)),
            _ => Outcome::Unavailable(channel),
        }
    }

    async fn display(&self, step: u8, up: bool) -> Outcome {
        let _serial = self.backlight.lock().await;
        let unavailable = Outcome::Unavailable(Channel::Brightness);
        let Some(light) = Backlight::discover(&self.backlights) else {
            return unavailable;
        };
        let level = light.stepped(step, up);
        if level != light.value {
            let Some(system) = Self::bus(&self.system, true).await else {
                return unavailable;
            };
            let set = light.set_level_at(&self.backlights, &system, level);
            if !matches!(
                tokio::time::timeout(SERVICE_DEADLINE, set).await,
                Ok(Ok(()))
            ) {
                self.forget(&self.system).await;
                return unavailable;
            }
        }
        Outcome::Changed(display_indicator(&Backlight {
            value: level,
            ..light
        }))
    }

    /// `Some(up)` steps; `None` toggles, turning back on at the level that
    /// toggling turned off (or the maximum).
    async fn keyboard(&self, up: Option<bool>) -> Outcome {
        let mut restore = self.backlight.lock().await;
        let unavailable = Outcome::Unavailable(Channel::KeyboardBacklight);
        let devices = KeyboardBacklight::discover_all(&self.leds);
        let Some(primary) = devices.first() else {
            return unavailable;
        };
        let Some(system) = Self::bus(&self.system, true).await else {
            return unavailable;
        };
        let mut shown = None;
        for device in &devices {
            let level = match up {
                Some(up) => device.stepped(up),
                None if device.value > 0 => {
                    restore.insert(device.name.clone(), device.value);
                    0
                }
                None => restore
                    .get(&device.name)
                    .copied()
                    .filter(|level| (1..=device.max).contains(level))
                    .unwrap_or(device.max),
            };
            let set = device.set_level_at(&self.leds, &system, level);
            let applied = level == device.value
                || matches!(
                    tokio::time::timeout(SERVICE_DEADLINE, set).await,
                    Ok(Ok(()))
                );
            if device.name == primary.name {
                if !applied {
                    self.forget(&self.system).await;
                    return unavailable;
                }
                shown = Some(keyboard_indicator(&KeyboardBacklight {
                    value: level,
                    ..device.clone()
                }));
            }
        }
        shown.map_or(unavailable, Outcome::Changed)
    }

    async fn media(&self, action: Action) -> Outcome {
        let Some(session) = Self::bus(&self.session, false).await else {
            return Outcome::NoPlayer;
        };
        let played = tokio::time::timeout(SERVICE_DEADLINE, media(&session, action)).await;
        if matches!(played, Ok(Ok(()))) {
            Outcome::Played
        } else {
            self.forget(&self.session).await;
            Outcome::NoPlayer
        }
    }
}

pub fn audio_indicator(channel: Channel, device: &audio::DefaultDevice) -> Indicator {
    Indicator::Level {
        channel,
        percent: device.percent,
        muted: device.muted,
        device: Some(device.description.clone()),
    }
}

pub fn display_indicator(light: &Backlight) -> Indicator {
    Indicator::Level {
        channel: Channel::Brightness,
        percent: light.perceptual(),
        muted: false,
        device: None,
    }
}

pub fn keyboard_indicator(light: &KeyboardBacklight) -> Indicator {
    Indicator::Level {
        channel: Channel::KeyboardBacklight,
        percent: light.percent(),
        muted: light.value == 0,
        device: None,
    }
}

/// The MPRIS operation for `action` on `player`: play-pause pauses a playing
/// player and plays any other.
fn operation(action: Action, player: &mpris::Player) -> Option<Operation> {
    Some(match action {
        Action::MediaPlayPause if player.status == "Playing" => Operation::Pause,
        Action::MediaPlayPause | Action::MediaPlay => Operation::Play,
        Action::MediaPause => Operation::Pause,
        Action::MediaStop => Operation::Stop,
        Action::MediaNext => Operation::Next,
        Action::MediaPrevious => Operation::Previous,
        _ => return None,
    })
    .filter(|operation| player.allows(*operation))
}

/// Control the Shell's selected player through `org.canopa.Shell.Media`, so
/// the bar, panel and keys agree. Without a running media source, pick the
/// player the Shell would pick automatically (Playing, then Paused).
async fn media(session: &zbus::Connection, action: Action) -> anyhow::Result<()> {
    use crate::media::{INTERFACE, PATH, Request as MediaRequest, SERVICE, Snapshot};
    let shell = zbus::Proxy::new(session, SERVICE, PATH, INTERFACE).await?;
    let selection = match shell.call::<_, _, String>("Snapshot", &()).await {
        Ok(json) => {
            anyhow::ensure!(json.len() <= 1024 * 1024, "media snapshot exceeds budget");
            Some(serde_json::from_str::<Snapshot>(&json)?)
        }
        // No media source in a running Shell: nobody has selected a player.
        Err(error) => match canopa_services::Error::from(error) {
            canopa_services::Error::Unavailable => None,
            error => return Err(error.into()),
        },
    };
    match selection {
        Some(snapshot) => {
            let player = snapshot
                .current()
                .ok_or_else(|| anyhow::anyhow!("no media player"))?;
            let operation =
                operation(action, player).ok_or_else(|| anyhow::anyhow!("unsupported control"))?;
            let request = MediaRequest::Control {
                player: player.id.clone(),
                track: player.track.clone(),
                operation,
            };
            shell
                .call::<_, _, ()>("Apply", &(serde_json::to_string(&request)?,))
                .await?;
        }
        None => {
            let mut snapshot = Snapshot::default();
            snapshot.reconcile(mpris::players(session).await?);
            let player = snapshot
                .current()
                .ok_or_else(|| anyhow::anyhow!("no media player"))?;
            let operation =
                operation(action, player).ok_or_else(|| anyhow::anyhow!("unsupported control"))?;
            mpris::control(session, &player.id, &player.track, operation).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
