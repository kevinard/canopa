//! Retain accepted images for asynchronous frontend export and host URI readers.
use super::Control;
use image::ImageEncoder;
use rustix::fs::{FlockOperation, Mode, OFlags};
use std::{
    fs::{self, File},
    io::{self, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

const PNG_BYTES: u64 = 128 * 1024 * 1024;
const CACHE_BYTES: u64 = 1024 * 1024 * 1024;
const CACHE_ENTRIES: usize = 128;
const MINIMUM_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone)]
pub(crate) struct Cache {
    home: Option<PathBuf>,
}
impl Cache {
    pub(crate) fn discover() -> Self {
        Self {
            home: std::env::var_os("XDG_CACHE_HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(PathBuf::from)
                        .filter(|p| p.is_absolute())
                        .map(|p| p.join(".cache"))
                }),
        }
    }
    #[cfg(test)]
    pub(super) fn new(home: PathBuf) -> Self {
        Self { home: Some(home) }
    }
    pub(super) fn publish(
        &self,
        pixels: &[u8],
        width: u32,
        height: u32,
        control: &Control,
    ) -> anyhow::Result<String> {
        control.check()?;
        let home = self
            .home
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Screenshot cache unavailable"))?;
        fs::create_dir_all(home)?;
        let mut path = home.clone();
        for (component, private) in [("canopa", false), ("portal", false), ("screenshots", true)] {
            path.push(component);
            directory(&path, private)?;
        }
        let anchor = File::from(rustix::fs::open(
            &path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let identity = anchor.metadata()?;
        let anchored = PathBuf::from(format!("/proc/self/fd/{}", anchor.as_raw_fd()));
        let lock = File::from(rustix::fs::open(
            anchored.join(".lock"),
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )?);
        anyhow::ensure!(
            ordinary(&lock.metadata()?) && lock.metadata()?.len() == 0,
            "Invalid screenshot cache lock"
        );
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            control.check()?;
            match rustix::fs::flock(&lock, FlockOperation::NonBlockingLockExclusive) {
                Ok(()) => break,
                Err(rustix::io::Errno::WOULDBLOCK) if Instant::now() < end => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(error) => return Err(error.into()),
            }
        }
        let mut temporary = tempfile::Builder::new()
            .prefix(".pending-")
            .rand_bytes(24)
            .tempfile_in(&anchored)?;
        let suffix = temporary
            .path()
            .file_name()
            .and_then(|v| v.to_str())
            .and_then(|v| v.strip_prefix(".pending-"))
            .ok_or_else(|| anyhow::anyhow!("Invalid screenshot temporary name"))?
            .to_owned();
        let filename = format!("{suffix}.png");
        let mut writer = Limited {
            file: temporary.as_file_mut(),
            bytes: 0,
            control,
        };
        image::codecs::png::PngEncoder::new_with_quality(
            &mut writer,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::Adaptive,
        )
        .write_image(pixels, width, height, image::ExtendedColorType::Rgba8)?;
        let size = writer.bytes;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o400))?;
        temporary.as_file().sync_all()?;
        let mut entries = Vec::new();
        for (index, entry) in fs::read_dir(&anchored)?.enumerate() {
            anyhow::ensure!(index < 256, "Screenshot cache entry limit exceeded");
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                anyhow::bail!("Invalid screenshot cache entry");
            };
            if name == ".lock" || name == format!(".pending-{suffix}") {
                continue;
            }
            if let Some(pending) = name.strip_prefix(".pending-") {
                anyhow::ensure!(
                    name.len() == 33
                        && pending.bytes().all(|v| v.is_ascii_alphanumeric())
                        && ordinary(&fs::symlink_metadata(entry.path())?),
                    "Invalid screenshot pending file"
                );
                fs::remove_file(entry.path())?;
            } else {
                anyhow::ensure!(
                    name.len() == 28
                        && name.ends_with(".png")
                        && name.as_bytes()[..24].iter().all(u8::is_ascii_alphanumeric),
                    "Invalid screenshot cache entry"
                );
                let metadata = fs::symlink_metadata(entry.path())?;
                anyhow::ensure!(ordinary(&metadata), "Invalid cached screenshot");
                entries.push((metadata.modified()?, metadata.len(), entry.path()));
            }
        }
        entries.sort_by_key(|entry| entry.0);
        let mut count = entries.len();
        let mut bytes: u64 = entries.iter().map(|entry| entry.1).sum();
        for (modified, length, candidate) in entries {
            control.check()?;
            if count < CACHE_ENTRIES && bytes + size <= CACHE_BYTES {
                break;
            }
            if SystemTime::now()
                .duration_since(modified)
                .unwrap_or_default()
                < MINIMUM_AGE
            {
                continue;
            }
            fs::remove_file(candidate)?;
            count -= 1;
            bytes -= length;
        }
        anyhow::ensure!(
            count < CACHE_ENTRIES && bytes + size <= CACHE_BYTES,
            "Recent screenshots fill the cache"
        );
        control.check()?;
        temporary
            .persist_noclobber(anchored.join(&filename))
            .map_err(|error| error.error)?;
        anchor.sync_all()?;
        let current = fs::symlink_metadata(&path)?;
        anyhow::ensure!(
            current.is_dir() && current.ino() == identity.ino() && current.dev() == identity.dev(),
            "Screenshot cache directory changed"
        );
        url::Url::from_file_path(path.join(filename))
            .map(String::from)
            .map_err(|_| anyhow::anyhow!("Invalid screenshot URI"))
    }
}
fn directory(path: &Path, private: bool) -> anyhow::Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    anyhow::ensure!(
        metadata.is_dir()
            && metadata.uid() == rustix::process::geteuid().as_raw()
            && metadata.mode() & if private { 0o077 } else { 0o022 } == 0,
        "Screenshot directory is not private"
    );
    Ok(())
}
fn ordinary(metadata: &fs::Metadata) -> bool {
    metadata.is_file()
        && metadata.uid() == rustix::process::geteuid().as_raw()
        && metadata.mode() & 0o077 == 0
        && metadata.nlink() == 1
        && metadata.len() <= PNG_BYTES
}
struct Limited<'a> {
    file: &'a mut File,
    bytes: u64,
    control: &'a Control,
}
impl Write for Limited<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.control.check().map_err(io::Error::other)?;
        if bytes.len() as u64 > PNG_BYTES - self.bytes {
            return Err(io::Error::other("Screenshot PNG exceeds limits"));
        }
        let count = self.file.write(bytes)?;
        self.bytes += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
