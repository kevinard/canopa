//! Where a path lives: a local disk, removable media or a network share, and
//! whether its filesystem has Unix permission bits. Thumbnail policy and the
//! trust given to executable bits both depend on it. Read on a worker.
use std::path::{Path, PathBuf};

const MAX_MOUNTINFO_BYTES: usize = 4 * 1024 * 1024;

/// Filesystems whose files live on another machine.
const NETWORK: &[&str] = &[
    "nfs",
    "nfs4",
    "cifs",
    "smb3",
    "smbfs",
    "ncpfs",
    "afs",
    "9p",
    "ceph",
    "glusterfs",
    "lustre",
    "gfs2",
    "ocfs2",
    "davfs",
    "fuse.sshfs",
    "fuse.rclone",
    "fuse.gvfsd-fuse",
    "fuse.s3fs",
    "fuse.davfs2",
    "fuse.curlftpfs",
    "fuse.glusterfs",
    "fuse.cephfs",
];

/// Filesystems whose mode bits are synthesized by mount options, so an
/// executable bit says nothing about the file.
const WITHOUT_PERMISSIONS: &[&str] = &[
    "vfat", "msdos", "exfat", "ntfs", "ntfs3", "fuseblk", "iso9660", "udf", "cifs", "smb3", "smbfs",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Local,
    Removable,
    Network,
}

/// What the containing mount says about a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MountFacts {
    pub placement: Placement,
    /// The filesystem stores Unix mode bits (not FAT, NTFS, ISO 9660 or SMB).
    pub unix_permissions: bool,
    /// Mounted `noexec`: nothing on it can be run.
    pub noexec: bool,
}

impl MountFacts {
    /// Unknown mounts are treated as foreign: no thumbnails by default, and no
    /// trust in executable bits.
    pub const UNKNOWN: Self = Self {
        placement: Placement::Network,
        unix_permissions: false,
        noexec: true,
    };

    pub fn local(self) -> bool {
        self.placement == Placement::Local
    }

    /// An executable bit here was set by a user, not by the filesystem.
    pub fn meaningful_executable_bit(self) -> bool {
        self.local() && self.unix_permissions && !self.noexec
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Mount {
    point: PathBuf,
    kind: String,
    source: String,
    noexec: bool,
}

/// One read of `/proc/self/mountinfo`.
#[derive(Clone, Debug, Default)]
pub struct Mounts {
    mounts: Vec<Mount>,
}

impl Mounts {
    pub fn read() -> Self {
        canopa_file::read_bounded(Path::new("/proc/self/mountinfo"), MAX_MOUNTINFO_BYTES)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .map(|source| Self::parse(&source))
            .unwrap_or_default()
    }

    pub fn parse(source: &str) -> Self {
        let mounts = source
            .lines()
            .filter_map(|line| {
                let (before, after) = line.split_once(" - ")?;
                let fields: Vec<&str> = before.split(' ').collect();
                let point = PathBuf::from(unescape(fields.get(4)?));
                let noexec = fields.get(5)?.split(',').any(|option| option == "noexec");
                let mut rest = after.split(' ');
                Some(Mount {
                    point,
                    kind: rest.next()?.to_owned(),
                    source: unescape(rest.next().unwrap_or_default()),
                    noexec,
                })
            })
            .collect();
        Self { mounts }
    }

    /// Facts for `path`, from the innermost mount containing it. Symlinks are
    /// resolved first when possible. Removable block devices are recognized
    /// from sysfs, and anything mounted under `/media` or `/run/media`.
    pub fn facts(&self, path: &Path) -> MountFacts {
        self.facts_with(path, &removable_device)
    }

    fn facts_with(&self, path: &Path, removable: &dyn Fn(&str) -> bool) -> MountFacts {
        let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
        // Later lines are newer mounts; the last, longest prefix covers the path.
        let Some(mount) = self
            .mounts
            .iter()
            .rev()
            .filter(|mount| resolved.starts_with(&mount.point))
            .max_by_key(|mount| mount.point.as_os_str().len())
        else {
            return MountFacts::UNKNOWN;
        };
        let network = NETWORK.contains(&mount.kind.as_str())
            || mount.source.starts_with("//")
            || (mount.kind.starts_with("nfs") && mount.source.contains(':'));
        let placement = if network {
            Placement::Network
        } else if mount.point.starts_with("/media")
            || mount.point.starts_with("/run/media")
            || mount.source.strip_prefix("/dev/").is_some_and(removable)
        {
            Placement::Removable
        } else {
            Placement::Local
        };
        MountFacts {
            placement,
            unix_permissions: !WITHOUT_PERMISSIONS.contains(&mount.kind.as_str()),
            noexec: mount.noexec,
        }
    }
}

/// `/sys/class/block/<device>`: its own or its disk's `removable` flag, or a
/// USB, MMC or FireWire bus in its device path.
fn removable_device(device: &str) -> bool {
    if device.is_empty() || device.contains("..") {
        return false;
    }
    let Ok(resolved) = std::fs::canonicalize(Path::new("/sys/class/block").join(device)) else {
        return false;
    };
    let text = resolved.to_string_lossy();
    if ["/usb", "/mmc_host/", "/firewire"]
        .iter()
        .any(|bus| text.contains(bus))
    {
        return true;
    }
    let flag = |dir: &Path| {
        std::fs::read_to_string(dir.join("removable")).is_ok_and(|value| value.trim() == "1")
    };
    flag(&resolved) || resolved.parent().is_some_and(flag)
}

/// mountinfo escapes space, tab, newline and backslash as octal.
fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\'
            && let Some(code) = bytes
                .get(index + 1..index + 4)
                .and_then(|digits| std::str::from_utf8(digits).ok())
                .and_then(|digits| u8::from_str_radix(digits, 8).ok())
        {
            output.push(code);
            index += 4;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTINFO: &str = concat!(
        "22 1 259:2 / / rw,relatime shared:1 - ext4 /dev/nvme0n1p2 rw\n",
        "30 22 0:26 / /home rw,relatime shared:2 - btrfs /dev/nvme0n1p3 rw\n",
        "31 22 8:17 / /run/media/me/STICK rw,nosuid,nodev - vfat /dev/sdb1 rw,fmask=0022\n",
        "32 22 8:33 / /mnt/backup rw - ext4 /dev/sdc1 rw\n",
        "33 30 0:50 / /home/me/nas rw - cifs //nas/share rw\n",
        "34 30 0:51 / /home/me/remote\\040box rw,noexec - fuse.sshfs me@box: rw\n",
        "35 22 0:52 / /tmp rw,nosuid,nodev,noexec - tmpfs tmpfs rw\n",
        "36 22 0:53 / /srv/nfs rw - nfs4 server:/export rw\n",
        "malformed line\n",
    );

    #[test]
    fn paths_take_the_innermost_mount_and_its_placement() {
        let mounts = Mounts::parse(MOUNTINFO);
        let facts = |path: &str| mounts.facts_with(Path::new(path), &|device| device == "sdc1");
        assert_eq!(
            facts("/home/me/Pictures/a.png"),
            MountFacts {
                placement: Placement::Local,
                unix_permissions: true,
                noexec: false
            }
        );
        assert!(facts("/home/me/Pictures").meaningful_executable_bit());
        let stick = facts("/run/media/me/STICK/run.sh");
        assert_eq!(stick.placement, Placement::Removable);
        assert!(!stick.unix_permissions && !stick.meaningful_executable_bit());
        assert_eq!(facts("/mnt/backup/x").placement, Placement::Removable);
        assert_eq!(facts("/home/me/nas/x").placement, Placement::Network);
        let remote = facts("/home/me/remote box/x");
        assert_eq!(remote.placement, Placement::Network);
        assert!(remote.noexec);
        assert_eq!(facts("/srv/nfs/x").placement, Placement::Network);
        let tmp = facts("/tmp/script.sh");
        assert!(tmp.local() && tmp.noexec && !tmp.meaningful_executable_bit());
        assert_eq!(
            Mounts::default().facts_with(Path::new("/x"), &|_| false),
            MountFacts::UNKNOWN
        );
    }
}
