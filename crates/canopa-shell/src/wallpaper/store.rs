use std::{
    fs::{self, File, OpenOptions},
    io::{self, Cursor, Read, Seek, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

use image::{ImageEncoder, ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(super) const MAX_PNG_BYTES: u64 = 128 * 1024 * 1024;
const MAX_PIXELS: u64 = 64 * 1024 * 1024;
const MAX_DIMENSION: u32 = 16_384;

#[derive(Debug, Clone, Copy)]
pub(super) struct Targets(u32);

impl Targets {
    pub(super) fn new(mask: u32) -> io::Result<Self> {
        match mask {
            1..=3 => Ok(Self(mask)),
            _ => Err(invalid("invalid wallpaper destinations")),
        }
    }

    pub(super) fn lockscreen(self) -> bool {
        self.0 & 2 != 0
    }
}

#[derive(Debug)]
pub(super) struct Prepared {
    png: Vec<u8>,
    pub(super) pixels: iced::widget::image::Handle,
}

impl Prepared {
    /// Private owner input is a canonical PNG, never an arbitrary caller path.
    pub(super) fn read(file: &mut File) -> io::Result<Self> {
        let bytes = read_regular(file, MAX_PNG_BYTES)?;
        let mut reader = ImageReader::with_format(Cursor::new(&bytes), ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        limits.max_alloc = Some(512 * 1024 * 1024);
        reader.limits(limits.clone());
        let (width, height) = reader.into_dimensions().map_err(invalid)?;
        if u64::from(width) * u64::from(height) > MAX_PIXELS || width == 0 || height == 0 {
            return Err(invalid("wallpaper dimensions exceed limits"));
        }
        let mut reader = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
        reader.limits(limits);
        let rgba = reader.decode().map_err(invalid)?.into_rgba8();
        let mut encoded = LimitedWriter(Vec::new());
        image::codecs::png::PngEncoder::new(&mut encoded)
            .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)
            .map_err(invalid)?;
        Ok(Self {
            png: encoded.0,
            pixels: iced::widget::image::Handle::from_rgba(width, height, rgba.into_raw()),
        })
    }
}

// The PNG encoder only needs Write; ImageEncoder avoids an unbounded Cursor.
struct LimitedWriter(Vec<u8>);
impl Write for LimitedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) as u64 > MAX_PNG_BYTES {
            return Err(invalid("encoded wallpaper exceeds limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Snapshot {
    pub(crate) background: Option<iced::widget::image::Handle>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    version: u32,
    background: Option<String>,
    lockscreen: Option<String>,
}

impl Selection {
    fn validate(&self) -> io::Result<()> {
        if self.version != 1
            || self
                .background
                .iter()
                .chain(self.lockscreen.iter())
                .any(|s| !is_hash(s))
        {
            return Err(invalid("invalid wallpaper selection"));
        }
        Ok(())
    }
}

pub(super) struct Store {
    root: PathBuf,
}

impl Store {
    pub(super) fn discover() -> io::Result<Self> {
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .map(|p| p.join(".local/state"))
            })
            .ok_or_else(|| invalid("wallpaper state directory unavailable"))?;
        Ok(Self {
            root: state.join("canopa/shell/wallpaper"),
        })
    }

    fn acquire(&self, create: bool, wait: bool) -> io::Result<Option<File>> {
        if !create && !self.root.try_exists()? {
            return Ok(None);
        }
        if create {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&self.root)?;
        }
        if !fs::symlink_metadata(&self.root)?.is_dir() {
            return Err(invalid("wallpaper state must be a directory"));
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(create)
            .mode(0o600)
            .custom_flags(no_follow())
            .open(self.root.join(".lock"))?;
        if !lock.metadata()?.is_file() {
            return Err(invalid("wallpaper lock must be regular"));
        }
        if wait {
            lock.lock()?;
        } else {
            lock.try_lock().map_err(io::Error::other)?;
        }
        Ok(Some(lock))
    }

    fn selection(&self) -> io::Result<Selection> {
        let mut file = match open_regular(&self.root.join("selection.json")) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Selection {
                    version: 1,
                    ..Selection::default()
                });
            }
            Err(error) => return Err(error),
        };
        let state: Selection =
            serde_json::from_slice(&read_regular(&mut file, 4096)?).map_err(invalid)?;
        state.validate()?;
        Ok(state)
    }

    fn image_path(&self, hash: &str) -> PathBuf {
        self.root.join(format!("{hash}.png"))
    }

    fn open_image(&self, hash: &str) -> io::Result<File> {
        let mut file = open_regular(&self.image_path(hash))?;
        let bytes = read_regular(&mut file, MAX_PNG_BYTES)?;
        if format!("{:x}", Sha256::digest(&bytes)) != hash {
            return Err(invalid("wallpaper image changed"));
        }
        file.rewind()?;
        Ok(file)
    }

    pub(super) fn snapshot(&self) -> io::Result<Snapshot> {
        let Some(_lock) = self.acquire(false, true)? else {
            return Ok(Snapshot::default());
        };
        let selected = self.selection()?;
        let background = selected
            .background
            .map(|hash| Prepared::read(&mut self.open_image(&hash)?).map(|p| p.pixels))
            .transpose()?;
        Ok(Snapshot { background })
    }

    /// Open under the same lock used by cleanup. The returned descriptor pins the inode.
    pub(super) fn lock_image(&self) -> io::Result<Option<File>> {
        let Some(_lock) = self.acquire(false, false)? else {
            return Ok(None);
        };
        self.selection()?
            .lockscreen
            .map(|hash| self.open_image(&hash))
            .transpose()
    }

    pub(super) fn install(&self, image: Prepared, targets: Targets) -> io::Result<Snapshot> {
        let _lock = self.acquire(true, true)?;
        let mut selection = self.selection()?;
        // Also reap an image left by an interrupted or failed prior publication.
        self.cleanup(&selection);
        let hash = format!("{:x}", Sha256::digest(&image.png));
        if targets.0 & 1 != 0 {
            selection.background = Some(hash.clone());
        }
        if targets.lockscreen() {
            selection.lockscreen = Some(hash.clone());
        }
        // Prepare retained background before committing so a corrupt retained image
        // cannot turn a lock-only change into a partially installed result.
        let background = if targets.0 & 1 != 0 {
            Some(image.pixels)
        } else {
            selection
                .background
                .as_ref()
                .map(|s| Prepared::read(&mut self.open_image(s)?).map(|p| p.pixels))
                .transpose()?
        };
        // Replacing the same hash also repairs a damaged on-disk copy.
        atomic_write(&self.root, &self.image_path(&hash), &image.png, 0o400)?;
        atomic_write(
            &self.root,
            &self.root.join("selection.json"),
            &serde_json::to_vec(&selection).map_err(invalid)?,
            0o600,
        )?;
        self.cleanup(&selection);
        Ok(Snapshot { background })
    }

    // A failed cleanup cannot invalidate a successfully published selection.
    fn cleanup(&self, selection: &Selection) {
        if let Ok(entries) = fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                // No active writer has a temporary file while this lock is held.
                if name.to_str().is_some_and(|n| n.starts_with(".pending-")) {
                    let _ = fs::remove_file(entry.path());
                    continue;
                }
                let Some(hash) = name.to_str().and_then(|n| n.strip_suffix(".png")) else {
                    continue;
                };
                if is_hash(hash)
                    && selection.background.as_deref() != Some(hash)
                    && selection.lockscreen.as_deref() != Some(hash)
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
}

fn atomic_write(root: &Path, target: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    let mut temp = tempfile::Builder::new()
        .prefix(".pending-")
        .tempfile_in(root)?;
    temp.write_all(bytes)?;
    temp.as_file()
        .set_permissions(fs::Permissions::from_mode(mode))?;
    temp.as_file().sync_all()?;
    temp.persist(target).map_err(|e| e.error)?;
    File::open(root)?.sync_all()
}

fn open_regular(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(no_follow())
        .open(path)
}

fn no_follow() -> i32 {
    (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32
}

fn read_regular(file: &mut File, maximum: u64) -> io::Result<Vec<u8>> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(invalid("wallpaper input must be a bounded regular file"));
    }
    file.rewind()?;
    let mut bytes = Vec::new();
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        return Err(invalid("wallpaper input exceeds limit"));
    }
    Ok(bytes)
}

fn is_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests;
