//! A single immutable client image shared by the backend and its metadata UI.
//! Discovery and copying never load CUPS into either process.
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::fs::{FileExt, MetadataExt, OpenOptionsExt},
    },
    path::Path,
    sync::{Arc, OnceLock},
};

const MAX_IMAGE_BYTES: u64 = 16 * 1024 * 1024;
const SEALS: rustix::fs::SealFlags = rustix::fs::SealFlags::SHRINK
    .union(rustix::fs::SealFlags::GROW)
    .union(rustix::fs::SealFlags::WRITE)
    .union(rustix::fs::SealFlags::SEAL);

/// Private inherited descriptor metadata, never a public Print option.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Selection {
    #[default]
    System,
    Image {
        descriptor: i32,
    },
    Unavailable,
}
impl Selection {
    pub(crate) fn is_system(&self) -> bool {
        matches!(self, Self::System)
    }
}

#[derive(Clone)]
pub(crate) enum Runtime {
    System,
    Image(Arc<File>),
    Unavailable,
}
impl Runtime {
    pub(crate) fn configured() -> Self {
        static RUNTIME: OnceLock<Runtime> = OnceLock::new();
        RUNTIME
            .get_or_init(|| {
                // Freeze the operator's choice once for this backend's lifetime.
                // No public request or printer metadata can select executable code.
                let explicit = std::env::var_os("CANOPA_PRINT_CUPS_LIBRARY");
                let bundled = std::env::current_exe().ok().and_then(|p| {
                    Some(p.parent()?.parent()?.join("lib/canopa/print/libcups.so.2"))
                });
                let path = explicit.as_deref().map(Path::new).or(bundled.as_deref());
                match path.map(Self::snapshot) {
                    None => Self::System,
                    Some(Ok(image)) => image,
                    Some(Err(error))
                        if explicit.is_none()
                            && error
                                .downcast_ref::<std::io::Error>()
                                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
                    {
                        Self::System
                    }
                    Some(Err(_)) => Self::Unavailable,
                }
            })
            .clone()
    }
    pub(crate) fn snapshot(path: &Path) -> anyhow::Result<Self> {
        let source = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        let before = source.metadata()?;
        anyhow::ensure!(
            before.is_file() && (4..=MAX_IMAGE_BYTES).contains(&before.len()),
            "Invalid CUPS client image"
        );
        let flags = rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING;
        // Ask explicitly for executable memory. Older kernels reject the new
        // flag with EINVAL; enforced no-exec policy must still fail visibly.
        let fd = match rustix::fs::memfd_create(
            c"canopa-print-client",
            flags | rustix::fs::MemfdFlags::EXEC,
        ) {
            Err(rustix::io::Errno::INVAL) => {
                rustix::fs::memfd_create(c"canopa-print-client", flags)?
            }
            result => result?,
        };
        let image = File::from(fd);
        let mut buffer = [0; 64 * 1024];
        let mut offset = 0;
        while offset < before.len() {
            let length = (before.len() - offset).min(buffer.len() as u64) as usize;
            source.read_exact_at(&mut buffer[..length], offset)?;
            image.write_all_at(&buffer[..length], offset)?;
            offset += length as u64;
        }
        let after = source.metadata()?;
        anyhow::ensure!(
            (
                before.len(),
                before.mtime(),
                before.mtime_nsec(),
                before.ctime(),
                before.ctime_nsec()
            ) == (
                after.len(),
                after.mtime(),
                after.mtime_nsec(),
                after.ctime(),
                after.ctime_nsec()
            ),
            "CUPS client changed during snapshot"
        );
        rustix::fs::fcntl_add_seals(&image, SEALS)?;
        validate(&image)?;
        Ok(Self::Image(Arc::new(image)))
    }
    pub(crate) fn receive(selection: Selection) -> anyhow::Result<Self> {
        Ok(match selection {
            Selection::System => Self::System,
            Selection::Unavailable => Self::Unavailable,
            Selection::Image { descriptor } => {
                let image = super::inherited(&[descriptor])?.remove(0);
                validate(&image)?;
                Self::Image(Arc::new(image))
            }
        })
    }
    pub(crate) fn inherit(&self, descriptors: &mut Vec<OwnedFd>) -> anyhow::Result<Selection> {
        Ok(match self {
            Self::System => Selection::System,
            Self::Unavailable => Selection::Unavailable,
            Self::Image(image) => {
                let descriptor = rustix::io::fcntl_dupfd_cloexec(&**image, 3)?;
                let selection = Selection::Image {
                    descriptor: descriptor.as_raw_fd(),
                };
                descriptors.push(descriptor);
                selection
            }
        })
    }
    pub(crate) fn has_image(&self) -> bool {
        matches!(self, Self::Image(_))
    }
}

fn validate(image: &File) -> anyhow::Result<()> {
    let metadata = image.metadata()?;
    anyhow::ensure!(
        metadata.is_file()
            && metadata.nlink() == 0
            && (4..=MAX_IMAGE_BYTES).contains(&metadata.len())
            && rustix::fs::fcntl_get_seals(image)?.contains(SEALS),
        "Invalid sealed CUPS client image"
    );
    let mut magic = [0; 4];
    image.read_exact_at(&mut magic, 0)?;
    anyhow::ensure!(magic == *b"\x7fELF", "Invalid CUPS client format");
    Ok(())
}

pub(super) struct Loaded {
    pub library: libloading::Library,
    pub limits: crate::print::authentication::ServerLimits,
    _image: Runtime,
}
static LOADED: OnceLock<Result<Loaded, String>> = OnceLock::new();

/// Only the standalone worker calls this, before any CUPS function is used.
pub(super) fn initialize(selection: Selection) -> anyhow::Result<()> {
    anyhow::ensure!(LOADED.get().is_none(), "CUPS client already selected");
    let runtime = Runtime::receive(selection)?;
    let value = load(runtime).map_err(|e| e.to_string());
    let _ = LOADED.set(value);
    library().map(|_| ())
}
fn load(runtime: Runtime) -> anyhow::Result<Loaded> {
    let path = match &runtime {
        Runtime::System => std::path::PathBuf::from("libcups.so.2"),
        Runtime::Image(file) => format!("/proc/self/fd/{}", file.as_raw_fd()).into(),
        Runtime::Unavailable => anyhow::bail!("Selected CUPS client runtime unavailable"),
    };
    // SAFETY: isolated worker only. The operator-selected, sealed image uses
    // opaque public APIs. Keep it and its library alive to process exit because
    // CUPS registers thread-local destructors.
    let library = unsafe { libloading::Library::new(path) }?;
    let limits = if runtime.has_image() {
        // SAFETY: our optional versioned extension has this C header signature.
        let version = unsafe {
            library.get::<unsafe extern "C" fn() -> u32>(b"canopaCupsAuthenticationVersion\0")?()
        };
        anyhow::ensure!(version == 1, "Unsupported CUPS authentication capability");
        crate::print::authentication::ServerLimits::CanopaV1
    } else {
        crate::print::authentication::ServerLimits::Legacy
    };
    Ok(Loaded {
        library,
        limits,
        _image: runtime,
    })
}
pub(super) fn library() -> anyhow::Result<&'static Loaded> {
    LOADED
        .get_or_init(|| load(Runtime::System).map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|message| anyhow::anyhow!("{message}"))
}

#[cfg(test)]
mod tests;
