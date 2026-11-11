//! Plain device observations; GIO discovery remains inside the owning helper.
use super::*;
pub use crate::network::LocationKind;

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
    /// Local storage, a network location, a phone or a camera.
    pub kind: LocationKind,
    /// The GIO location, for display, bookmarks and the GIO listing fallback.
    /// Never a request target and never holds a password.
    pub uri: Option<String>,
    /// Mounted by GVFS, but without a local folder: its FUSE daemon is not
    /// running. Only the read-only GIO listing can show it.
    pub remote_only: bool,
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
    #[serde(default)]
    pub kind: LocationKind,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub remote_only: bool,
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
            kind: self.kind,
            uri: self.uri,
            remote_only: self.remote_only,
            mount: self.mount.and_then(|instance| {
                roots.get(&instance).map(|mount| MountToken {
                    instance,
                    epoch: mount.epoch,
                })
            }),
        }
    }
}

/// Mounts the parent observes as Storage roots: unshadowed, with a local folder
/// (for GVFS locations, the FUSE path).
pub(super) fn observed_mounts(monitor: &gio::VolumeMonitor) -> BTreeSet<u64> {
    monitor
        .mounts()
        .into_iter()
        .filter(|m| !m.is_shadowed() && m.root().path().is_some())
        .map(|m| m.as_ptr() as usize as u64)
        .collect()
}

/// A URI worth showing: bounded, single-line and without user information
/// beyond a user name (GVFS never puts passwords in mount URIs).
fn shown_uri(uri: glib::GString) -> Option<String> {
    let uri = uri.to_string();
    (uri.len() <= 4096 && !uri.chars().any(char::is_control)).then_some(uri)
}

pub(super) fn collect(monitor: &gio::VolumeMonitor, observed: &BTreeSet<u64>) -> Vec<WireDevice> {
    let volumes: Vec<_> = monitor
        .volumes()
        .into_iter()
        .filter(|volume| volume.get_mount().is_none_or(|mount| !mount.is_shadowed()))
        .collect();
    let mut devices: Vec<_> = volumes
        .iter()
        .map(|volume| {
            let mount = volume.get_mount();
            let uri = mount
                .as_ref()
                .map(|mount| mount.root().uri())
                .or_else(|| volume.activation_root().map(|root| root.uri()))
                .and_then(shown_uri);
            let kind = match uri.as_deref().map(LocationKind::of_uri) {
                Some(kind) if kind.remote() => kind,
                _ if volume.identifier("class").as_deref() == Some("network") => {
                    LocationKind::Network
                }
                _ => LocationKind::Local,
            };
            let observed_mount = mount
                .as_ref()
                .map(|mount| mount.as_ptr() as usize as u64)
                .filter(|instance| observed.contains(instance));
            WireDevice {
                instance: volume.as_ptr() as usize as u64,
                name: volume.name().into(),
                mount: observed_mount,
                can_mount: mount.is_none() && volume.can_mount(),
                removal: removal(volume.drive(), volume.can_eject(), mount.as_ref()),
                kind,
                remote_only: mount.is_some() && observed_mount.is_none(),
                uri,
            }
        })
        .collect();
    devices.extend(
        monitor
            .mounts()
            .into_iter()
            .filter(|mount| {
                !mount.is_shadowed()
                    && mount
                        .volume()
                        .is_none_or(|volume| !volumes.contains(&volume))
            })
            .filter_map(|mount| {
                let instance = mount.as_ptr() as usize as u64;
                let uri = shown_uri(mount.root().uri());
                let kind = uri
                    .as_deref()
                    .map_or(LocationKind::Local, LocationKind::of_uri);
                let local = observed.contains(&instance);
                // A mount without a local folder is only useful when it is a
                // GVFS location the read-only GIO listing can still show.
                if !local && (!kind.remote() || uri.is_none()) {
                    return None;
                }
                Some(WireDevice {
                    instance,
                    name: mount.name().into(),
                    mount: local.then_some(instance),
                    can_mount: false,
                    removal: removal(mount.drive(), false, Some(&mount)),
                    kind,
                    uri,
                    remote_only: !local,
                })
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
