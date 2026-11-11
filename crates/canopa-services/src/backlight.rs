//! The built-in display backlight in sysfs, set through logind.
//!
//! sysfs gives the current and maximum level; logind's `SetBrightness` on the
//! session object writes it without root. There is no change signal for
//! brightness, so [`watch`] polls.
//!
//! ```no_run
//! use canopa_services::{ServiceState, backlight::{self, Backlight}};
//! # async fn page() -> Result<(), canopa_services::Error> {
//! let system = canopa_services::bus::system().await?;
//! match Backlight::discover(std::path::Path::new(backlight::ROOT)) {
//!     Some(light) => {
//!         println!("{} %", light.percent());
//!         light.set(&system, 60).await?; // Revalidates the device first.
//!     }
//!     None => println!("No built-in backlight"),
//! }
//! let mut changes = backlight::watch(std::time::Duration::from_secs(3));
//! # let _ = &mut changes;
//! # Ok(()) }
//! ```
use crate::{Error, ServiceState, bus::StateStream, logind::Session};
use std::{path::Path, time::Duration};

/// Where the kernel lists backlight devices.
pub const ROOT: &str = "/sys/class/backlight";

/// One backlight device with its levels at discovery time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backlight {
    /// The sysfs device name, for example `intel_backlight`.
    pub name: String,
    pub max: u32,
    pub value: u32,
}

impl Backlight {
    /// The preferred device under `root`: firmware, then platform, then raw
    /// interfaces, by name within a type. Devices with no range are skipped.
    pub fn discover(root: &Path) -> Option<Self> {
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

    /// The level as a rounded percentage.
    pub fn percent(&self) -> u16 {
        ((u64::from(self.value) * 100 + u64::from(self.max) / 2) / u64::from(self.max)) as u16
    }

    /// The hardware level for `percent`, rounded and never 0 so the panel stays lit.
    pub fn level(&self, percent: u16) -> u32 {
        ((u64::from(self.max) * u64::from(percent) + 50) / 100).max(1) as u32
    }

    /// Set 1–100 % on this session's logind session. Fails with
    /// [`Error::Changed`] when the device under [`ROOT`] is no longer this one.
    pub async fn set(&self, connection: &zbus::Connection, percent: u16) -> Result<(), Error> {
        self.set_at(Path::new(ROOT), connection, percent).await
    }

    /// [`Backlight::set`] revalidating against the devices under `root`.
    pub async fn set_at(
        &self,
        root: &Path,
        connection: &zbus::Connection,
        percent: u16,
    ) -> Result<(), Error> {
        if !(1..=100).contains(&percent) {
            return Err(Error::Invalid("Brightness must be between 1% and 100%."));
        }
        let current = Self::discover(root).ok_or(Error::Changed)?;
        if self.name != current.name || self.max != current.max {
            return Err(Error::Changed);
        }
        let session = Session::find(connection).await?;
        session
            .set_brightness("backlight", &self.name, self.level(percent))
            .await
    }
}

/// Poll [`ROOT`] every `period`, yielding the device when it changes and
/// [`ServiceState::Unavailable`] while there is none.
pub fn watch(period: Duration) -> StateStream<Backlight> {
    watch_at(Path::new(ROOT).to_owned(), period)
}

/// [`watch`] under another directory.
pub fn watch_at(root: std::path::PathBuf, period: Duration) -> StateStream<Backlight> {
    let mut ticks = tokio::time::interval(period);
    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    Box::pin(futures_util::stream::unfold(
        (ticks, root, None::<ServiceState<Backlight>>),
        |(mut ticks, root, mut last)| async move {
            loop {
                ticks.tick().await;
                let current = ServiceState::from(Backlight::discover(&root));
                if last.as_ref() != Some(&current) {
                    last = Some(current.clone());
                    return Some((current, (ticks, root, last)));
                }
            }
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestBus, logind::FakeLogind};
    use futures_util::StreamExt;

    /// Write a device atomically, so a concurrent poll never sees half of it.
    fn device(root: &Path, name: &str, kind: &str, max: &str, brightness: &str) {
        let staging = tempfile::tempdir_in(root).unwrap();
        let path = staging.path().join(name);
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("type"), kind).unwrap();
        std::fs::write(path.join("max_brightness"), max).unwrap();
        std::fs::write(path.join("brightness"), brightness).unwrap();
        let target = root.join(name);
        if target.exists() {
            std::fs::remove_dir_all(&target).unwrap();
        }
        std::fs::rename(&path, &target).unwrap();
    }

    #[test]
    fn selects_firmware_backlight_and_uses_hardware_range() {
        let root = tempfile::tempdir().unwrap();
        for (name, kind, max, brightness) in [
            ("intel", "raw", "100", "75"),
            ("acpi", "firmware", "400", "100"),
            ("bad", "firmware", "0", "0"),
        ] {
            device(root.path(), name, kind, max, brightness);
        }
        let light = Backlight::discover(root.path()).unwrap();
        assert_eq!(light.name, "acpi");
        assert_eq!(light.percent(), 25);
        assert_eq!(light.level(1), 4);
        assert_eq!(light.level(0), 1);
    }

    #[tokio::test]
    async fn set_revalidates_the_device_and_asks_the_session() {
        let root = tempfile::tempdir().unwrap();
        device(root.path(), "acpi", "firmware", "400", "100");
        let light = Backlight::discover(root.path()).unwrap();
        let bus = TestBus::new();
        let logind = FakeLogind::default();
        let _server = logind.serve(&bus).await;
        let client = bus.connect().await;
        assert_eq!(
            light.set_at(root.path(), &client, 0).await,
            Err(Error::Invalid("Brightness must be between 1% and 100%."))
        );
        light.set_at(root.path(), &client, 50).await.unwrap();
        assert_eq!(
            *logind.brightness.lock().unwrap(),
            [("backlight".to_string(), "acpi".to_string(), 200)]
        );
        device(root.path(), "acpi", "firmware", "800", "100");
        assert_eq!(
            light.set_at(root.path(), &client, 50).await,
            Err(Error::Changed)
        );
    }

    #[tokio::test]
    async fn watch_reports_appearance_changes_and_removal() {
        let root = tempfile::tempdir().unwrap();
        let mut states = watch_at(root.path().to_owned(), Duration::from_millis(10));
        assert_eq!(states.next().await, Some(ServiceState::Unavailable));
        device(root.path(), "acpi", "firmware", "400", "100");
        assert!(matches!(
            states.next().await,
            Some(ServiceState::Available(Backlight { value: 100, .. }))
        ));
        device(root.path(), "acpi", "firmware", "400", "200");
        assert!(matches!(
            states.next().await,
            Some(ServiceState::Available(Backlight { value: 200, .. }))
        ));
        std::fs::remove_dir_all(root.path().join("acpi")).unwrap();
        assert_eq!(states.next().await, Some(ServiceState::Unavailable));
    }
}
