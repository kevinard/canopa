//! Publish reviewed PNG bytes beyond the lifetime of a dialog helper.
use super::Avatar;
use anyhow::{Context, bail, ensure};
use rustix::fs::{FlockOperation, Mode, OFlags};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Write,
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, MetadataExt},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

const PNG_BYTES: usize = 1024 * 1024;
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

#[derive(Clone, Copy)]
struct Policy {
    entries: usize,
    bytes: u64,
    minimum_age: Duration,
}

const POLICY: Policy = Policy {
    entries: 64,
    bytes: 16 * 1024 * 1024,
    minimum_age: Duration::from_secs(60),
};

pub(crate) fn publish(avatar: &Avatar) -> anyhow::Result<String> {
    let home = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|path| path.join(".cache"))
        })
        .context("local image cache is unavailable")?;
    publish_in(&home, &avatar.png, POLICY)
}

fn directory(path: &Path, private: bool) -> anyhow::Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_dir()
            && metadata.uid() == rustix::process::geteuid().as_raw()
            && metadata.mode() & if private { 0o077 } else { 0o022 } == 0,
        "image cache directory is not private"
    );
    Ok(())
}

fn ordinary(metadata: &fs::Metadata) -> bool {
    metadata.is_file()
        && metadata.uid() == rustix::process::geteuid().as_raw()
        && metadata.mode() & 0o077 == 0
        && metadata.nlink() == 1
        && metadata.len() <= PNG_BYTES as u64
}

fn image_name(name: &[u8]) -> bool {
    name.len() == 68
        && name.ends_with(b".png")
        && name[..64]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn pending_name(name: &[u8]) -> bool {
    name.strip_prefix(b".pending-")
        .is_some_and(|tail| tail.len() == 12 && tail.iter().all(u8::is_ascii_alphanumeric))
}

fn lock(directory: &Path) -> anyhow::Result<File> {
    let file = File::from(rustix::fs::open(
        directory.join(".lock"),
        OFlags::RDWR | OFlags::CREATE | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::RUSR | Mode::WUSR,
    )?);
    ensure!(ordinary(&file.metadata()?), "invalid image cache lock");
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match rustix::fs::flock(&file, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => return Ok(file),
            Err(rustix::io::Errno::WOULDBLOCK) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn publish_in(home: &Path, png: &[u8], policy: Policy) -> anyhow::Result<String> {
    ensure!(
        home.is_absolute() && png.starts_with(PNG_MAGIC) && png.len() <= PNG_BYTES,
        "invalid prepared image"
    );
    fs::create_dir_all(home)?;
    let mut path = home.to_owned();
    for (component, private) in [("canopa", false), ("portal", false), ("account", true)] {
        path.push(component);
        directory(&path, private)?;
    }
    let anchor = File::from(rustix::fs::open(
        &path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )?);
    let identity = anchor.metadata()?;
    let _lock = lock(&path)?;
    let digest = format!("{:x}", Sha256::digest(png));
    let destination = path.join(format!("{digest}.png"));
    let existing = match fs::symlink_metadata(&destination) {
        Ok(metadata) => {
            ensure!(
                ordinary(&metadata)
                    && canopa_file::read_canonical_bounded(&destination, PNG_BYTES)?.as_slice()
                        == png,
                "cached image differs from its reviewed bytes"
            );
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error.into()),
    };
    if existing {
        File::open(&destination)?.set_modified(SystemTime::now())?;
    } else {
        let mut entries = Vec::new();
        for (index, entry) in fs::read_dir(&path)?.enumerate() {
            ensure!(index < 256, "image cache entry limit exceeded");
            let entry = entry?;
            let name = entry.file_name();
            let bytes = name.as_bytes();
            if !image_name(bytes) && !pending_name(bytes) {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path())?;
            ensure!(ordinary(&metadata), "invalid cached image entry");
            if pending_name(bytes) {
                // All cooperating writers hold this lock; these are abandoned
                // writes from a helper which ended before publication.
                fs::remove_file(entry.path())?;
            } else {
                entries.push((metadata.modified()?, metadata.len(), entry.path()));
            }
        }
        entries.sort_by_key(|(modified, _, _)| *modified);
        let mut bytes: u64 = entries.iter().map(|(_, bytes, _)| *bytes).sum();
        let mut count = entries.len();
        for (modified, size, candidate) in entries {
            if count < policy.entries && bytes + png.len() as u64 <= policy.bytes {
                break;
            }
            if SystemTime::now()
                .duration_since(modified)
                .unwrap_or_default()
                < policy.minimum_age
            {
                continue;
            }
            fs::remove_file(candidate)?;
            bytes -= size;
            count -= 1;
        }
        if count >= policy.entries || bytes + png.len() as u64 > policy.bytes {
            bail!("recent shared images fill the cache");
        }
        let mut temporary = tempfile::Builder::new()
            .prefix(".pending-")
            .rand_bytes(12)
            .tempfile_in(&path)?;
        temporary.write_all(png)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist_noclobber(&destination)
            .map_err(|error| error.error)?;
        anchor.sync_all()?;
    }
    let current = fs::symlink_metadata(&path)?;
    ensure!(
        current.is_dir() && current.dev() == identity.dev() && current.ino() == identity.ino(),
        "image cache directory changed"
    );
    url::Url::from_file_path(destination)
        .map(String::from)
        .map_err(|_| anyhow::anyhow!("invalid image cache URI"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;

    fn png(color: u8) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                &[color, 80, 190, 255],
                1,
                1,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        bytes
    }
    fn path(uri: &str) -> PathBuf {
        url::Url::parse(uri).unwrap().to_file_path().unwrap()
    }

    #[test]
    fn shared_bytes_are_private_reusable_and_survive_the_preparation_owner() {
        use std::os::unix::ffi::OsStringExt;
        let root = tempfile::tempdir().unwrap();
        let cache = root
            .path()
            .join(std::ffi::OsString::from_vec(b"cache-\xff".to_vec()));
        let bytes = png(20);
        let uri = publish_in(&cache, &bytes, POLICY).unwrap();
        assert!(uri.contains("%FF"));
        assert_eq!(fs::read(path(&uri)).unwrap(), bytes);
        assert_eq!(fs::metadata(path(&uri)).unwrap().mode() & 0o777, 0o600);
        assert_eq!(publish_in(&cache, &bytes, POLICY).unwrap(), uri);
        let identity = fs::metadata(path(&uri)).unwrap().ino();
        assert_eq!(
            fs::metadata(path(&publish_in(&cache, &bytes, POLICY).unwrap()))
                .unwrap()
                .ino(),
            identity
        );
    }

    #[test]
    fn bounds_keep_recent_results_and_retire_only_old_generated_images() {
        let root = tempfile::tempdir().unwrap();
        let policy = Policy {
            entries: 1,
            bytes: PNG_BYTES as u64,
            minimum_age: Duration::from_secs(60),
        };
        let first = path(&publish_in(root.path(), &png(10), policy).unwrap());
        let unrelated = first.parent().unwrap().join("personal-note.txt");
        fs::write(&unrelated, b"keep this").unwrap();
        assert!(publish_in(root.path(), &png(30), policy).is_err());
        assert_eq!(fs::read(&first).unwrap(), png(10));
        File::open(&first)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(120))
            .unwrap();
        let second = path(&publish_in(root.path(), &png(30), policy).unwrap());
        assert_ne!(first, second);
        assert!(!first.exists());
        assert_eq!(fs::read(second).unwrap(), png(30));
        assert_eq!(fs::read(unrelated).unwrap(), b"keep this");
    }

    #[test]
    fn symlinks_and_changed_cached_content_never_redirect_or_overwrite_publication() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("canopa")).unwrap();
        assert!(publish_in(root.path(), &png(10), POLICY).is_err());
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
        fs::remove_file(root.path().join("canopa")).unwrap();
        let target = path(&publish_in(root.path(), &png(10), POLICY).unwrap());
        fs::write(&target, b"changed cache content").unwrap();
        assert!(publish_in(root.path(), &png(10), POLICY).is_err());
        assert_eq!(fs::read(target).unwrap(), b"changed cache content");
    }

    #[test]
    fn concurrent_shares_publish_one_complete_copy() {
        let root = tempfile::tempdir().unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads: Vec<_> = (0..2)
            .map(|_| {
                let directory = root.path().to_owned();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    publish_in(&directory, &png(20), POLICY).unwrap()
                })
            })
            .collect();
        let uris: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect();
        assert_eq!(uris[0], uris[1]);
        assert_eq!(fs::read(path(&uris[0])).unwrap(), png(20));
    }
}
