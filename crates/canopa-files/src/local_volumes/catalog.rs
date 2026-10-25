//! Plain device observations; GIO discovery remains inside the owning helper.
use super::*;

/// An observed device in this helper lifetime. Neither labels nor paths are IDs.
/// The revision expires on the next platform observation; it is not persisted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DeviceToken {
    pub(super) instance: u64,
    pub(super) revision: u64,
}

/// Stable row identity while this device remains observed. Not a request target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(pub(super) u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Removal {
    PowerOff,
    Eject,
    Unmount,
}

impl Removal {
    pub fn label(self) -> &'static str {
        match self {
            Self::PowerOff => "Power off",
            Self::Eject => "Eject",
            Self::Unmount => "Unmount",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub token: DeviceToken,
    pub name: String,
    pub root: Option<PathBuf>,
    pub can_mount: bool,
    pub removal: Option<Removal>,
    pub(super) mount: Option<MountToken>,
    pub(super) id: DeviceId,
}

impl Device {
    pub fn id(&self) -> DeviceId {
        self.id
    }
}

/// Prepared parent observation, independent of unrelated catalog revisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MountToken {
    pub instance: u64,
    pub epoch: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DeviceStatus {
    #[default]
    Starting,
    Ready,
    Stopped,
    Failed(String),
}

#[derive(Clone, Debug, Default)]
pub struct DeviceSnapshot {
    pub generation: u64,
    pub status: DeviceStatus,
    pub devices: Vec<Device>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireDevice {
    pub instance: u64,
    pub name: String,
    pub mount: Option<u64>,
    pub can_mount: bool,
    pub removal: Option<Removal>,
}

impl WireDevice {
    pub fn prepare(self, generation: u64, roots: &Observed, id: DeviceId) -> Device {
        Device {
            id,
            token: DeviceToken {
                instance: self.instance,
                revision: generation,
            },
            name: self.name,
            root: self
                .mount
                .and_then(|instance| roots.get(&instance).map(|mount| mount.path.clone())),
            can_mount: self.can_mount,
            removal: self.removal,
            mount: self.mount.and_then(|instance| {
                roots.get(&instance).map(|mount| MountToken {
                    instance,
                    epoch: mount.epoch,
                })
            }),
        }
    }
}

pub(super) fn collect(monitor: &gio::VolumeMonitor, observed: &BTreeSet<u64>) -> Vec<WireDevice> {
    let volumes: Vec<_> = monitor
        .volumes()
        .into_iter()
        .filter(|volume| {
            volume.identifier("class").as_deref() != Some("network")
                && volume
                    .activation_root()
                    .is_none_or(|root| root.path().is_some())
                && volume
                    .get_mount()
                    .is_none_or(|mount| !mount.is_shadowed() && mount.root().path().is_some())
        })
        .collect();
    let mut devices: Vec<_> = volumes
        .iter()
        .map(|volume| {
            let mount = volume.get_mount();
            WireDevice {
                instance: volume.as_ptr() as usize as u64,
                name: volume.name().into(),
                mount: mount
                    .as_ref()
                    .map(|mount| mount.as_ptr() as usize as u64)
                    .filter(|instance| observed.contains(instance)),
                can_mount: mount.is_none() && volume.can_mount(),
                removal: removal(volume.drive(), volume.can_eject(), mount.as_ref()),
            }
        })
        .collect();
    devices.extend(
        monitor
            .mounts()
            .into_iter()
            .filter(|mount| {
                !mount.is_shadowed()
                    && observed.contains(&(mount.as_ptr() as usize as u64))
                    && mount.root().path().is_some()
                    && mount
                        .volume()
                        .is_none_or(|volume| !volumes.contains(&volume))
            })
            .map(|mount| WireDevice {
                instance: mount.as_ptr() as usize as u64,
                name: mount.name().into(),
                mount: Some(mount.as_ptr() as usize as u64),
                can_mount: false,
                removal: removal(mount.drive(), false, Some(&mount)),
            }),
    );
    devices.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.instance.cmp(&b.instance))
    });
    let mut mounted = BTreeSet::new();
    devices.retain(|device| device.mount.is_none_or(|instance| mounted.insert(instance)));
    devices
}

fn removal(
    drive: Option<gio::Drive>,
    volume_eject: bool,
    mount: Option<&gio::Mount>,
) -> Option<Removal> {
    if drive.as_ref().is_some_and(|drive| drive.can_stop()) {
        Some(Removal::PowerOff)
    } else if drive.as_ref().is_some_and(|drive| drive.can_eject())
        || volume_eject
        || mount.is_some_and(|mount| mount.can_eject())
    {
        Some(Removal::Eject)
    } else if mount.is_some_and(|mount| mount.can_unmount()) {
        Some(Removal::Unmount)
    } else {
        None
    }
}
