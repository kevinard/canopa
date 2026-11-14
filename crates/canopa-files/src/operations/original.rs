//! A descriptor-derived, persistent object identity. This is evidence only:
//! callers still need the exact request, placement and mutation ownership.
use super::*;
use std::{
    ffi::{c_char, c_int},
    os::fd::AsRawFd,
    sync::OnceLock,
};

// Linux 6.8: unlike STATX_MNT_ID this value is never reused during one boot.
// rustix 1.1 does not yet name the flag; unsupported kernels must refuse proof.
const MNT_ID_UNIQUE: u32 = 0x0000_4000;
const MAX_HANDLE: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Wire", into = "Wire")]
pub(in crate::operations) struct Original {
    boot: [u8; 16],
    mount: u64,
    object: [u8; 32],
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    version: u8,
    #[serde(serialize_with = "serialized::bytes")]
    boot: [u8; 16],
    mount: u64,
    #[serde(serialize_with = "serialized::bytes")]
    object: [u8; 32],
}

impl From<Original> for Wire {
    fn from(value: Original) -> Self {
        Self {
            version: 1,
            boot: value.boot,
            mount: value.mount,
            object: value.object,
        }
    }
}
impl TryFrom<Wire> for Original {
    type Error = &'static str;
    fn try_from(value: Wire) -> Result<Self, Self::Error> {
        if value.version != 1
            || value.mount == 0
            || value.boot == [0; 16]
            || value.object == [0; 32]
        {
            return Err("invalid original-entry proof");
        }
        Ok(Self {
            boot: value.boot,
            mount: value.mount,
            object: value.object,
        })
    }
}

#[repr(C)]
struct KernelHandle {
    bytes: u32,
    kind: c_int,
    data: [u8; MAX_HANDLE],
}
unsafe extern "C" {
    fn name_to_handle_at(
        fd: c_int,
        path: *const c_char,
        handle: *mut KernelHandle,
        mount: *mut c_int,
        flags: c_int,
    ) -> c_int;
}

impl Original {
    pub(in crate::operations) fn capture(file: &fs::File) -> io::Result<Self> {
        Self::capture_from(file, &Self::status(file, rustix::fs::StatxFlags::empty())?)
    }

    /// The object's identity, and its proof when the filesystem has one, from
    /// one `statx`: [`Identity::from_metadata`] of `file` and [`Self::capture`].
    pub(in crate::operations) fn identify(
        file: &fs::File,
    ) -> io::Result<(Identity, io::Result<Self>)> {
        let status = Self::status(file, rustix::fs::StatxFlags::INO)?;
        let identity = Identity {
            device: rustix::fs::makedev(status.stx_dev_major, status.stx_dev_minor),
            inode: status.stx_ino,
        };
        Ok((identity, Self::capture_from(file, &status)))
    }

    fn status(file: &fs::File, also: rustix::fs::StatxFlags) -> io::Result<rustix::fs::Statx> {
        use rustix::fs::{AtFlags, StatxFlags, statx};
        Ok(statx(
            file,
            c"",
            AtFlags::EMPTY_PATH | AtFlags::SYMLINK_NOFOLLOW,
            StatxFlags::from_bits_retain(MNT_ID_UNIQUE) | also,
        )?)
    }

    fn capture_from(file: &fs::File, status: &rustix::fs::Statx) -> io::Result<Self> {
        use rustix::fs::AtFlags;
        let boot = boot()?;
        if status.stx_mask & MNT_ID_UNIQUE == 0 || status.stx_mnt_id == 0 {
            return Err(unsupported("This filesystem has no unique mount identity."));
        }
        let mut handle = KernelHandle {
            bytes: MAX_HANDLE as u32,
            kind: 0,
            data: [0; MAX_HANDLE],
        };
        let mut mount = 0;
        // The fixed buffer includes MAX_HANDLE_SZ bytes after the C header.
        // AT_EMPTY_PATH names the opened object, preserving final symlinks and
        // avoiding any fresh pathname observation, file read or device I/O.
        let result = unsafe {
            name_to_handle_at(
                file.as_raw_fd(),
                c"".as_ptr(),
                &mut handle,
                &mut mount,
                AtFlags::EMPTY_PATH.bits() as c_int,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        let size = handle.bytes as usize;
        if size == 0 || size > MAX_HANDLE {
            return Err(unsupported(
                "This filesystem has no bounded persistent file identity.",
            ));
        }
        // Parents and the history file recur throughout Undo. Keep obtaining
        // their current kernel handles and reuse only SHA-256 of exact bytes.
        let mut bytes = [0; 8 + MAX_HANDLE];
        bytes[..4].copy_from_slice(&handle.kind.to_le_bytes());
        bytes[4..8].copy_from_slice(&handle.bytes.to_le_bytes());
        bytes[8..8 + size].copy_from_slice(&handle.data[..size]);
        Ok(Self {
            boot,
            mount: status.stx_mnt_id,
            object: serialized::digest(&bytes[..8 + size]),
        })
    }

    pub(in crate::operations) fn require(&self, file: &fs::File) -> io::Result<()> {
        self.require_equal(&Self::capture(file)?)
    }

    /// [`Self::require`] against a proof already captured.
    pub(in crate::operations) fn require_equal(&self, current: &Self) -> io::Result<()> {
        if current == self {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "The original item or filesystem has changed. Its current contents are preserved.",
            ))
        }
    }
}

fn unsupported(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, message.to_owned())
}
fn boot() -> io::Result<[u8; 16]> {
    static BOOT: OnceLock<Result<[u8; 16], String>> = OnceLock::new();
    BOOT.get_or_init(|| {
        let text =
            fs::read_to_string("/proc/sys/kernel/random/boot_id").map_err(|e| e.to_string())?;
        let text = text.trim();
        if text.len() != 36
            || [8, 13, 18, 23]
                .into_iter()
                .any(|i| text.as_bytes()[i] != b'-')
        {
            return Err("The system boot identity is unavailable.".into());
        }
        let digits: String = text.chars().filter(|c| *c != '-').collect();
        if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("The system boot identity is unavailable.".into());
        }
        let mut value = [0; 16];
        for (i, byte) in value.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&digits[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string())?;
        }
        if value == [0; 16] {
            return Err("The system boot identity is unavailable.".into());
        }
        Ok(value)
    })
    .as_ref()
    .copied()
    .map_err(|e| unsupported(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    fn opened(path: &Path) -> fs::File {
        fs::File::from(
            rustix::fs::open(
                path,
                rustix::fs::OFlags::PATH
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .unwrap(),
        )
    }
    #[test]
    fn descriptor_proof_survives_serialization_and_refuses_replacements() {
        let mut verified = 0;
        for workspace in [false, true] {
            let root = if workspace {
                tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap()
            } else {
                tempfile::tempdir().unwrap()
            };
            for kind in ["file", "directory", "link", "fifo"] {
                let path = root.path().join(kind);
                let parked = root.path().join(format!("{kind}-original"));
                match kind {
                    "file" => fs::write(&path, b"original").unwrap(),
                    "directory" => fs::create_dir(&path).unwrap(),
                    "link" => symlink("missing", &path).unwrap(),
                    _ => rustix::fs::mkfifoat(rustix::fs::CWD, &path, rustix::fs::Mode::RUSR)
                        .unwrap(),
                }
                let file = opened(&path);
                let original = match Original::capture(&file) {
                    Ok(original) => original,
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::Unsupported | io::ErrorKind::PermissionDenied
                        ) =>
                    {
                        eprintln!(
                            "Original-entry proof unavailable on this test filesystem: {error}"
                        );
                        continue;
                    }
                    Err(error) => panic!("unexpected original-entry capture failure: {error}"),
                };
                let encoded = serde_json::to_vec(&original).unwrap();
                let restored: Original = serde_json::from_slice(&encoded).unwrap();
                drop(file);
                fs::rename(&path, &parked).unwrap();
                fs::write(&path, b"replacement").unwrap();
                assert_eq!(
                    restored.require(&opened(&path)).unwrap_err().kind(),
                    io::ErrorKind::WouldBlock
                );
                fs::remove_file(&path).unwrap();
                fs::rename(&parked, &path).unwrap();
                restored.require(&opened(&path)).unwrap();
                verified += 1;
            }
        }
        eprintln!("Verified {verified} durable original-entry replacement/return cases.");
    }
    #[test]
    fn invalid_and_foreign_scope_proofs_cannot_match_a_current_object() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("entry");
        fs::write(&path, b"bytes").unwrap();
        let file = opened(&path);
        let Ok(original) = Original::capture(&file) else {
            return;
        };
        let mut foreign = original;
        foreign.mount = foreign.mount.wrapping_add(1);
        assert_eq!(
            foreign.require(&file).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        foreign = original;
        foreign.boot[0] ^= 1;
        assert_eq!(
            foreign.require(&file).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        for key in ["version", "mount", "boot", "object"] {
            let mut wire = serde_json::to_value(original).unwrap();
            wire[key] = match key {
                "boot" => serde_json::json!(vec![0u8; 16]),
                "object" => serde_json::json!(vec![0u8; 32]),
                _ => serde_json::json!(0),
            };
            assert!(serde_json::from_value::<Original>(wire).is_err());
        }
    }
}
