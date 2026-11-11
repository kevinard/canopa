//! The built-in display backlight and keyboard backlights in sysfs, set
//! through logind.
//!
//! sysfs gives the current and maximum level; logind's `SetBrightness` on the
//! session object writes it without root. There is no change signal for
//! brightness, so [`watch`] polls. Firmware that changes a level by itself
//! (brightness hotkeys handled in firmware) notifies [`hardware_changes`];
//! writes by other programs do not.
//!
//! Brightness keys step on a perceptual curve ([`Backlight::stepped`]): equal
//! presses look like equal changes, and the lowest press stays lit.
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
use futures_util::stream::{self, BoxStream, StreamExt};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

/// Where the kernel lists backlight devices.
pub const ROOT: &str = "/sys/class/backlight";
/// Where the kernel lists LEDs, including keyboard backlights.
pub const LEDS: &str = "/sys/class/leds";
/// Perceived brightness is the hardware fraction to this power's root.
const GAMMA: f64 = 2.0;

fn read_level(path: &Path) -> Option<(u32, u32)> {
    let read = |file: &str| -> Option<u32> {
        std::fs::read_to_string(path.join(file))
            .ok()?
            .trim()
            .parse()
            .ok()
    };
    let (max, value) = (read("max_brightness")?, read("brightness")?);
    (max > 0 && value <= max).then_some((max, value))
}

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
                let (max, value) = read_level(&path)?;
                Some((
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

    /// Perceived brightness, 0–100: the level on a gamma-2 curve, so that
    /// equal key presses look like equal changes.
    pub fn perceptual(&self) -> u16 {
        (perceived(self.value, self.max) * 100.0).round() as u16
    }

    /// The hardware level that looks like `percent` (1–100) on the same
    /// curve as [`Backlight::perceptual`], so a slider moves evenly. Never 0.
    pub fn perceptual_level(&self, percent: u16) -> u32 {
        let fraction = f64::from(percent.min(100)) / 100.0;
        ((f64::from(self.max) * fraction.powf(GAMMA)).round() as u32).clamp(1, self.max.max(1))
    }

    /// Set a perceived brightness, 1–100, as a slider shows it
    /// ([`Backlight::perceptual`]), revalidating the device first.
    pub async fn set_perceptual(
        &self,
        connection: &zbus::Connection,
        percent: u16,
    ) -> Result<(), Error> {
        self.set_perceptual_at(Path::new(ROOT), connection, percent)
            .await
    }

    /// [`Backlight::set_perceptual`] revalidating against the devices under `root`.
    pub async fn set_perceptual_at(
        &self,
        root: &Path,
        connection: &zbus::Connection,
        percent: u16,
    ) -> Result<(), Error> {
        if !(1..=100).contains(&percent) {
            return Err(Error::Invalid("Brightness must be between 1% and 100%."));
        }
        self.set_level_at(root, connection, self.perceptual_level(percent))
            .await
    }

    /// The lowest level a key press reaches: 1 % of the range and at least 1,
    /// so the panel never goes dark by accident.
    pub fn minimum(&self) -> u32 {
        self.max.div_ceil(100).max(1)
    }

    /// The level one key press away: `step` perceptual percent up or down,
    /// at least one hardware level, between [`Backlight::minimum`] and the
    /// maximum. A level already below the minimum is never raised by a
    /// press down.
    pub fn stepped(&self, step: u8, up: bool) -> u32 {
        let step = f64::from(step.clamp(1, 100)) / 100.0;
        let current = perceived(self.value, self.max);
        let target = if up { current + step } else { current - step }.clamp(0.0, 1.0);
        let level = (f64::from(self.max) * target.powf(GAMMA)).round() as u32;
        if up {
            level.max(self.value.saturating_add(1)).min(self.max)
        } else {
            level
                .min(self.value.saturating_sub(1))
                .max(self.minimum().min(self.value))
        }
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
        self.set_level_at(root, connection, self.level(percent))
            .await
    }

    /// Set a raw level, 1 to the maximum, as [`Backlight::set`] does.
    pub async fn set_level(&self, connection: &zbus::Connection, level: u32) -> Result<(), Error> {
        self.set_level_at(Path::new(ROOT), connection, level).await
    }

    /// [`Backlight::set_level`] revalidating against the devices under `root`.
    pub async fn set_level_at(
        &self,
        root: &Path,
        connection: &zbus::Connection,
        level: u32,
    ) -> Result<(), Error> {
        if !(1..=self.max).contains(&level) {
            return Err(Error::Invalid("Brightness must be between 1% and 100%."));
        }
        let current = Self::discover(root).ok_or(Error::Changed)?;
        if self.name != current.name || self.max != current.max {
            return Err(Error::Changed);
        }
        let session = Session::find(connection).await?;
        session.set_brightness("backlight", &self.name, level).await
    }

    /// The file the kernel notifies when firmware changes this backlight.
    pub fn notifications(&self, root: &Path) -> PathBuf {
        root.join(&self.name).join("actual_brightness")
    }
}

fn perceived(value: u32, max: u32) -> f64 {
    (f64::from(value) / f64::from(max.max(1)))
        .clamp(0.0, 1.0)
        .powf(1.0 / GAMMA)
}

/// One keyboard backlight LED (`*::kbd_backlight`). Off (0) is a valid level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyboardBacklight {
    /// The sysfs LED name, for example `tpacpi::kbd_backlight`.
    pub name: String,
    pub max: u32,
    pub value: u32,
}

impl KeyboardBacklight {
    /// Every keyboard backlight under `root` (normally [`LEDS`]), by name.
    pub fn discover_all(root: &Path) -> Vec<Self> {
        let mut found: Vec<Self> = std::fs::read_dir(root)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                if !name.ends_with("::kbd_backlight") || name.len() > 255 {
                    return None;
                }
                let (max, value) = read_level(&entry.path())?;
                Some(Self { name, max, value })
            })
            .collect();
        found.sort_by(|a, b| a.name.cmp(&b.name));
        found
    }

    /// The level as a rounded percentage.
    pub fn percent(&self) -> u16 {
        ((u64::from(self.value) * 100 + u64::from(self.max) / 2) / u64::from(self.max.max(1)))
            as u16
    }

    /// The level one key press away: one hardware level on keyboards with a
    /// few levels, a tenth of the range on finer ones.
    pub fn stepped(&self, up: bool) -> u32 {
        let step = if self.max <= 10 {
            1
        } else {
            self.max.div_ceil(10)
        };
        if up {
            self.value.saturating_add(step).min(self.max)
        } else {
            self.value.saturating_sub(step)
        }
    }

    /// Set a level from 0 to the maximum through this session's logind
    /// session, revalidating the device under `root` first.
    pub async fn set_level_at(
        &self,
        root: &Path,
        connection: &zbus::Connection,
        level: u32,
    ) -> Result<(), Error> {
        if level > self.max {
            return Err(Error::Invalid(
                "Keyboard brightness is above the device's range.",
            ));
        }
        let path = root.join(&self.name);
        match read_level(&path) {
            Some((max, _)) if max == self.max => {}
            _ => return Err(Error::Changed),
        }
        let session = Session::find(connection).await?;
        session.set_brightness("leds", &self.name, level).await
    }

    /// The file the kernel notifies when firmware changes this LED by
    /// itself, if the driver supports it.
    pub fn notifications(&self, root: &Path) -> PathBuf {
        root.join(&self.name).join("brightness_hw_changed")
    }
}

/// Yield a file from `files` each time the kernel notifies it
/// (`sysfs_notify`). Files that are missing or cannot be watched are skipped;
/// the stream then waits forever rather than ending. It wakes only on
/// notifications.
pub fn hardware_changes(files: Vec<PathBuf>) -> BoxStream<'static, PathBuf> {
    use std::os::unix::fs::FileExt;
    use tokio::io::{Interest, unix::AsyncFd};
    let watches = files.into_iter().filter_map(|path| {
        let file = std::fs::File::open(&path).ok()?;
        // Reading arms the notification.
        let mut buffer = [0_u8; 64];
        file.read_at(&mut buffer, 0).ok()?;
        // SAFETY: the `File` owns its descriptor, which stays open and the
        // same for as long as the `AsyncFd` owns the `File`.
        let watched = unsafe { AsyncFd::register_with_interest(file, Interest::PRIORITY) }.ok()?;
        Some(
            stream::unfold((watched, path), |(watched, path)| async move {
                let mut guard = watched.ready(Interest::PRIORITY).await.ok()?;
                guard.clear_ready();
                drop(guard);
                let mut buffer = [0_u8; 64];
                watched.get_ref().read_at(&mut buffer, 0).ok()?;
                Some((path.clone(), (watched, path)))
            })
            .boxed(),
        )
    });
    stream::select_all(watches).chain(stream::pending()).boxed()
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

    #[test]
    fn steps_are_perceptual_move_every_press_and_never_go_dark() {
        let light = |max, value| Backlight {
            name: "acpi".into(),
            max,
            value,
        };
        let fine = light(96000, 96000);
        assert_eq!(fine.perceptual(), 100);
        let mut level = fine.value;
        let mut seen = Vec::new();
        for _ in 0..30 {
            level = light(96000, level).stepped(5, false);
            seen.push(light(96000, level).perceptual());
        }
        // Equal presses take equal perceptual steps down to the floor.
        assert_eq!(&seen[..3], [95, 90, 85]);
        assert_eq!(level, fine.minimum());
        assert_eq!(fine.minimum(), 960);
        assert_eq!(light(96000, 960).stepped(5, false), 960);
        assert!(light(96000, 960).perceptual() > 0);
        // Up from the floor and capped at the maximum.
        assert!(light(96000, 960).stepped(5, true) > 960);
        assert_eq!(light(96000, 95000).stepped(5, true), 96000);
        assert_eq!(light(96000, 96000).stepped(5, true), 96000);

        // Coarse ranges still move one level per press.
        assert_eq!(light(7, 7).stepped(5, false), 6);
        assert_eq!(light(7, 1).stepped(5, false), 1);
        assert_eq!(light(7, 1).stepped(5, true), 2);
        // Something else set 0: a press down leaves it alone.
        assert_eq!(light(100, 0).stepped(5, false), 0);
        assert!(light(100, 0).stepped(5, true) >= 1);
    }

    #[tokio::test]
    async fn sliders_use_the_perceptual_curve_both_ways() {
        let light = |value| Backlight {
            name: "acpi".into(),
            max: 400,
            value,
        };
        // Half of the perceived range is a quarter of the hardware range.
        assert_eq!(light(0).perceptual_level(50), 100);
        assert_eq!(light(100).perceptual(), 50);
        assert_eq!(light(0).perceptual_level(100), 400);
        assert_eq!(light(0).perceptual_level(1), 1);
        for percent in [5, 20, 50, 73, 100] {
            assert_eq!(
                light(light(0).perceptual_level(percent)).perceptual(),
                percent
            );
        }
        let root = tempfile::tempdir().unwrap();
        device(root.path(), "acpi", "firmware", "400", "100");
        let bus = TestBus::new();
        let logind = FakeLogind::default();
        let _server = logind.serve(&bus).await;
        let client = bus.connect().await;
        light(100)
            .set_perceptual_at(root.path(), &client, 50)
            .await
            .unwrap();
        assert!(matches!(
            light(100).set_perceptual_at(root.path(), &client, 0).await,
            Err(Error::Invalid(_))
        ));
        assert_eq!(logind.brightness.lock().unwrap()[0].2, 100);
    }

    #[tokio::test]
    async fn keyboard_backlights_step_toggle_levels_and_revalidate() {
        let root = tempfile::tempdir().unwrap();
        device(root.path(), "tpacpi::kbd_backlight", "", "2", "1");
        device(root.path(), "input3::capslock", "", "1", "0");
        device(root.path(), "asus::kbd_backlight", "", "255", "0");
        let found = KeyboardBacklight::discover_all(root.path());
        assert_eq!(
            found.iter().map(|k| k.name.as_str()).collect::<Vec<_>>(),
            ["asus::kbd_backlight", "tpacpi::kbd_backlight"]
        );
        let (fine, coarse) = (&found[0], &found[1]);
        assert_eq!((coarse.stepped(true), coarse.stepped(false)), (2, 0));
        assert_eq!(coarse.percent(), 50);
        assert_eq!(fine.stepped(true), 26);
        assert_eq!(fine.stepped(false), 0);

        let bus = TestBus::new();
        let logind = FakeLogind::default();
        let _server = logind.serve(&bus).await;
        let client = bus.connect().await;
        coarse.set_level_at(root.path(), &client, 0).await.unwrap();
        assert_eq!(
            coarse.set_level_at(root.path(), &client, 3).await,
            Err(Error::Invalid(
                "Keyboard brightness is above the device's range."
            ))
        );
        assert_eq!(
            *logind.brightness.lock().unwrap(),
            [("leds".to_string(), "tpacpi::kbd_backlight".to_string(), 0)]
        );
        device(root.path(), "tpacpi::kbd_backlight", "", "3", "1");
        assert_eq!(
            coarse.set_level_at(root.path(), &client, 1).await,
            Err(Error::Changed)
        );
        assert_eq!(
            coarse.notifications(root.path()),
            root.path()
                .join("tpacpi::kbd_backlight/brightness_hw_changed")
        );
    }

    #[tokio::test]
    async fn display_levels_are_set_raw_and_revalidated() {
        let root = tempfile::tempdir().unwrap();
        device(root.path(), "acpi", "firmware", "400", "100");
        let light = Backlight::discover(root.path()).unwrap();
        let bus = TestBus::new();
        let logind = FakeLogind::default();
        let _server = logind.serve(&bus).await;
        let client = bus.connect().await;
        for refused in [0, 401] {
            assert!(matches!(
                light.set_level_at(root.path(), &client, refused).await,
                Err(Error::Invalid(_))
            ));
        }
        light
            .set_level_at(root.path(), &client, light.stepped(5, false))
            .await
            .unwrap();
        assert_eq!(logind.brightness.lock().unwrap()[0].2, 81);
    }

    #[tokio::test]
    async fn unwatchable_notification_files_are_skipped_without_ending() {
        let root = tempfile::tempdir().unwrap();
        // Ordinary files cannot be watched; missing files are skipped.
        std::fs::write(root.path().join("actual_brightness"), "5\n").unwrap();
        let mut changes = hardware_changes(vec![
            root.path().join("actual_brightness"),
            root.path().join("missing"),
        ]);
        assert!(
            tokio::time::timeout(Duration::from_millis(50), changes.next())
                .await
                .is_err()
        );
    }
}
