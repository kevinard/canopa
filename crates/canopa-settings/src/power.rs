//! The Power page's models: the session's lid-close choice (a file the
//! Canopa session follows) and the live power state from UPower,
//! power-profiles-daemon and the backlight.
//!
//! The lid choice is applied by the session, not by this window: the display
//! profile watcher, which runs for the whole Canopa session, follows
//! `power.kdl` and holds logind's lid inhibitor only for a choice other than
//! the system setting (ADR 0140).
use crate::{
    live::{LiveAction, LiveService, ServiceError},
    registry::Document,
    session::Paths,
    storage::Snapshot,
};
use anyhow::{Context, Result, ensure};
use canopa_services::{
    ServiceState,
    backlight::{self, Backlight},
    power_profiles::{self, PowerProfiles},
    power_switches::LidAction,
    upower::{self, PowerSupply},
};
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream::BoxStream};
use std::{path::PathBuf, sync::Arc, time::Duration};

/// The choices in `power.kdl`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PowerChoices {
    pub lid_close: LidAction,
}

impl PowerChoices {
    pub fn parse(text: &str) -> Result<Self> {
        let document: kdl::KdlDocument = if text.trim().is_empty() {
            kdl::KdlDocument::new()
        } else {
            text.parse()?
        };
        let mut choices = Self::default();
        for node in document.nodes() {
            match node.name().value() {
                "lid-close" => {
                    let id = node
                        .get(0)
                        .and_then(|value| value.as_string())
                        .context("lid-close needs an action")?;
                    choices.lid_close = LidAction::from_id(id)
                        .with_context(|| format!("unknown lid action {id}"))?;
                }
                other => anyhow::bail!("power.kdl has an unknown setting `{other}`"),
            }
        }
        Ok(choices)
    }

    pub fn text(&self) -> String {
        format!(
            "// Managed by Canopa Settings; the Canopa session follows it.\nlid-close {}\n",
            crate::storage::quote(self.lid_close.id())
        )
    }

    /// The saved choices, or the defaults when the file is missing or invalid.
    pub fn load(paths: &Paths) -> Self {
        Snapshot::load(paths.power())
            .ok()
            .and_then(|source| Self::parse(&source.text).ok())
            .unwrap_or_default()
    }
}

impl crate::file_page::FileModel for PowerChoices {
    const DOCUMENT: Document = Document::Power;
    const READ_ONLY: &'static str =
        "power.kdl must be an owned regular file; symlinked configuration is read-only";

    fn path(paths: &Paths) -> PathBuf {
        paths.power()
    }
    fn read(source: &Snapshot) -> Result<Self> {
        Self::parse(&source.text)
    }
    fn fallback() -> Self {
        Self::default()
    }
    fn write(&self, _: &Paths, source: &Snapshot) -> Result<Snapshot> {
        let text = self.text();
        source.save(&text, |path| {
            Self::parse(&std::fs::read_to_string(path)?)?;
            Ok(())
        })
    }
}

/// Follow `power.kdl` for the session and apply its lid choice through
/// logind. Runs until the system bus or the file watch fails.
pub async fn follow_lid(paths: Paths) -> Result<()> {
    use canopa_file::watch::{Target, Watcher};
    let system = canopa_services::bus::system()
        .await
        .map_err(|error| anyhow::anyhow!("lid choice: {error}"))?;
    let (choice, policy) = tokio::sync::watch::channel(PowerChoices::load(&paths).lid_close);
    tokio::spawn(canopa_services::power_switches::handle_lid(system, policy));
    let watcher = Watcher::new(vec![Target::File(paths.power())])?;
    // SAFETY: Watcher owns its inotify descriptor and never replaces it.
    let mut file =
        unsafe { tokio::io::unix::AsyncFd::register(watcher) }.map_err(std::io::Error::from)?;
    loop {
        crate::profile_watch::file_changed(&mut file).await?;
        choice.send_if_modified(|current| {
            let next = PowerChoices::load(&paths).lid_close;
            std::mem::replace(current, next) != next
        });
    }
}

/// What the Power page shows from its services. Each part is `None` while
/// its service is not available, which is an ordinary state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PowerState {
    pub supply: Option<PowerSupply>,
    pub profiles: Option<PowerProfiles>,
    pub backlight: Option<Backlight>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PowerAction {
    Profile(String),
    /// Perceived brightness, 1–100.
    Brightness(u16),
}

impl LiveAction for PowerAction {
    fn control(&self) -> String {
        match self {
            Self::Profile(_) => "power-profile",
            Self::Brightness(_) => "brightness",
        }
        .into()
    }
}

/// UPower, power-profiles-daemon and the built-in backlight for the page.
#[derive(Clone)]
pub struct PowerService {
    system: Arc<tokio::sync::OnceCell<zbus::Connection>>,
    backlights: PathBuf,
}

impl Default for PowerService {
    fn default() -> Self {
        Self {
            system: Arc::default(),
            backlights: backlight::ROOT.into(),
        }
    }
}

const NO_BUS: &str = "The system bus is not reachable.";

impl PowerService {
    /// A service on an existing connection, with backlights under `root`.
    pub fn with(system: zbus::Connection, root: PathBuf) -> Self {
        Self {
            system: Arc::new(tokio::sync::OnceCell::new_with(Some(system))),
            backlights: root,
        }
    }

    async fn connection(&self) -> Result<zbus::Connection, ServiceError> {
        self.system
            .get_or_try_init(canopa_services::bus::system)
            .await
            .cloned()
            .map_err(|_| ServiceError::Unavailable(NO_BUS.into()))
    }

    async fn read(&self) -> Result<PowerState, ServiceError> {
        let system = self.connection().await?;
        let (supply, profiles) = tokio::join!(
            upower::read_power_supply(&system),
            PowerProfiles::discover(&system),
        );
        Ok(PowerState {
            supply: supply.ok(),
            profiles,
            backlight: Backlight::discover(&self.backlights),
        })
    }
}

enum Part {
    Supply(ServiceState<PowerSupply>),
    Profiles(ServiceState<PowerProfiles>),
    Backlight(ServiceState<Backlight>),
}

impl LiveService for PowerService {
    type State = PowerState;
    type Action = PowerAction;

    fn name(&self) -> &str {
        "The power services"
    }

    fn snapshot(&self) -> BoxFuture<'static, Result<PowerState, ServiceError>> {
        let service = self.clone();
        async move { service.read().await }.boxed()
    }

    fn updates(&self) -> BoxStream<'static, Result<PowerState, ServiceError>> {
        let service = self.clone();
        futures_util::stream::once(async move {
            let Ok(system) = service.connection().await else {
                return futures_util::stream::iter([Err(ServiceError::Unavailable(NO_BUS.into()))])
                    .boxed();
            };
            let parts = futures_util::stream::select_all([
                upower::watch_power_supply(system.clone())
                    .map(Part::Supply)
                    .boxed(),
                power_profiles::watch(system).map(Part::Profiles).boxed(),
                backlight::watch_at(service.backlights.clone(), Duration::from_secs(2))
                    .map(Part::Backlight)
                    .boxed(),
            ]);
            parts
                .scan(PowerState::default(), |state, part| {
                    match part {
                        Part::Supply(value) => state.supply = value.available().cloned(),
                        Part::Profiles(value) => state.profiles = value.available().cloned(),
                        Part::Backlight(value) => state.backlight = value.available().cloned(),
                    }
                    std::future::ready(Some(Ok(state.clone())))
                })
                .boxed()
        })
        .flatten()
        .boxed()
    }

    fn apply(
        &self,
        action: PowerAction,
    ) -> BoxFuture<'static, Result<Option<PowerState>, ServiceError>> {
        let service = self.clone();
        async move {
            let system = service.connection().await?;
            let current = service.read().await?;
            let result = match &action {
                PowerAction::Profile(profile) => {
                    let profiles = current.profiles.ok_or_else(|| {
                        ServiceError::Failed("Power modes are not available.".into())
                    })?;
                    profiles.set(&system, profile).await
                }
                PowerAction::Brightness(percent) => {
                    let light = current.backlight.ok_or_else(|| {
                        ServiceError::Failed("There is no built-in display backlight.".into())
                    })?;
                    light
                        .set_perceptual_at(&service.backlights, &system, *percent)
                        .await
                }
            };
            result.map_err(|error| ServiceError::Failed(failure(&action, &error)))?;
            Ok(Some(service.read().await?))
        }
        .boxed()
    }
}

fn failure(action: &PowerAction, error: &canopa_services::Error) -> String {
    use canopa_services::Error;
    match (action, error) {
        (_, Error::NotAuthorized) => "The system did not allow this change.".into(),
        (_, Error::Changed) => "The device changed. Try again.".into(),
        (_, Error::TimedOut) => "The service did not answer in time.".into(),
        (PowerAction::Brightness(_), Error::Invalid(message)) => (*message).into(),
        (PowerAction::Brightness(_), _) => {
            "The brightness could not be changed in this session.".into()
        }
        (PowerAction::Profile(_), _) => "The power mode could not be changed.".into(),
    }
}

/// Seconds as words: "30 seconds", "5 minutes", "1 hour 30 minutes".
pub fn duration_label(seconds: u32) -> String {
    let plural =
        |value: u32, unit: &str| format!("{value} {unit}{}", if value == 1 { "" } else { "s" });
    let (hours, minutes, rest) = (seconds / 3600, seconds % 3600 / 60, seconds % 60);
    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(plural(hours, "hour"));
    }
    if minutes > 0 {
        parts.push(plural(minutes, "minute"));
    }
    if rest > 0 || parts.is_empty() {
        parts.push(plural(rest, "second"));
    }
    parts.join(" ")
}

/// The durations offered for each idle stage, in seconds. A saved value that
/// is not one of them is offered too.
pub const DURATIONS: [u32; 14] = [
    15, 30, 60, 120, 180, 300, 600, 900, 1200, 1800, 2700, 3600, 7200, 10800,
];

/// What a lid choice is called on the page.
pub fn lid_label(action: LidAction) -> &'static str {
    match action {
        LidAction::System => "Use the system setting",
        LidAction::Suspend => "Suspend",
        LidAction::Hibernate => "Hibernate",
        LidAction::Lock => "Lock the screen",
        LidAction::Nothing => "Do nothing",
        LidAction::PowerOff => "Power off",
    }
}

/// Check a choice the page offers before writing it.
pub fn validate(choices: &PowerChoices) -> Result<()> {
    ensure!(
        LidAction::ALL.contains(&choices.lid_close),
        "unknown lid action"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use canopa_services::testing::{
        TestBus, logind::FakeLogind, power_profiles::FakePowerProfiles, upower::FakeUPower,
    };

    #[test]
    fn choices_round_trip_and_reject_unknown_settings() {
        for action in LidAction::ALL {
            let choices = PowerChoices { lid_close: action };
            assert_eq!(PowerChoices::parse(&choices.text()).unwrap(), choices);
        }
        assert_eq!(PowerChoices::parse("").unwrap(), PowerChoices::default());
        assert!(PowerChoices::parse("lid-close \"dance\"\n").is_err());
        assert!(PowerChoices::parse("other 1\n").is_err());
        assert_eq!(duration_label(300), "5 minutes");
        assert_eq!(duration_label(5400), "1 hour 30 minutes");
        assert_eq!(duration_label(45), "45 seconds");
        assert_eq!(duration_label(3601), "1 hour 1 second");
    }

    fn backlight(root: &std::path::Path, value: u32) {
        let path = root.join("acpi");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("type"), "firmware").unwrap();
        std::fs::write(path.join("max_brightness"), "400").unwrap();
        std::fs::write(path.join("brightness"), value.to_string()).unwrap();
    }

    #[tokio::test]
    async fn the_page_reads_and_changes_each_service_and_tolerates_missing_ones() {
        let bus = TestBus::new();
        let root = tempfile::tempdir().unwrap();
        let service = PowerService::with(bus.connect().await, root.path().into());
        // Nothing is running: every part is simply absent.
        let state = service.snapshot().await.unwrap();
        assert_eq!(state, PowerState::default());
        assert!(matches!(
            service.apply(PowerAction::Brightness(50)).await,
            Err(ServiceError::Failed(_))
        ));

        let logind = FakeLogind::default();
        let _logind = logind.serve(&bus).await;
        let _upower = FakeUPower::serve(&bus).await;
        let daemon = FakePowerProfiles::serve(&bus, 0, &["balanced", "power-saver"]).await;
        backlight(root.path(), 100);
        let state = service.snapshot().await.unwrap();
        assert_eq!(state.supply.as_ref().unwrap().batteries.len(), 1);
        assert_eq!(state.profiles.as_ref().unwrap().active, "balanced");
        assert_eq!(state.backlight.as_ref().unwrap().perceptual(), 50);

        let mut updates = service.updates();
        let first = tokio::time::timeout(Duration::from_secs(3), updates.next())
            .await
            .unwrap();
        assert!(matches!(first, Some(Ok(_))));

        service
            .apply(PowerAction::Profile("power-saver".into()))
            .await
            .unwrap();
        assert_eq!(daemon.active(), "power-saver");
        assert!(matches!(
            service.apply(PowerAction::Profile("performance".into())).await,
            Err(ServiceError::Failed(message)) if message.contains("changed")
        ));
        // 75 % perceived is 56 % of the hardware range: 225 of 400.
        service.apply(PowerAction::Brightness(75)).await.unwrap();
        assert_eq!(logind.brightness.lock().unwrap()[0].2, 225);
    }
}
