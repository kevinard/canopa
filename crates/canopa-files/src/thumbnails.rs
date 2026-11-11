//! Thumbnails for listed files, following the freedesktop Thumbnail Managing
//! Standard: the shared cache first, then built-in decoding of common images,
//! then an installed external thumbnailer in a bounded Bubblewrap sandbox.
//!
//! [`Generator::thumbnail`] is synchronous; callers run it on admitted workers
//! and cancel it when the listing it serves goes away. Failures of a real
//! attempt are recorded under `fail/`, so a file is not retried until it
//! changes or Files is upgraded.
use crate::{
    Cancellation,
    mounts::{MountFacts, Mounts},
    preview::Bitmap,
};
use canopa_assets::thumbnails::{
    Cache, Flavor, Lookup, MAX_THUMBNAIL_BYTES, Source, Thumbnailer, Thumbnailers, encode,
    resolve_program,
};
use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
};
use canopa_file::mime::MimeDatabase;
use std::{
    os::{
        fd::{AsRawFd, BorrowedFd},
        unix::process::CommandExt,
    },
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// The standard's `<appname>-<version>` for failure records.
pub const APPLICATION: &str = concat!("canopa-files-", env!("CARGO_PKG_VERSION"));
const BUBBLEWRAP: &str = "/usr/bin/bwrap";
/// Types decoded in-process by the memory-safe `image` codecs Files enables.
const BUILT_IN: &[&str] = &["image/png", "image/jpeg", "image/gif", "image/webp"];
const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
const SANDBOX_TMP_BYTES: u64 = 64 * 1024 * 1024;

pub use crate::interface::ThumbnailPolicy;

/// Bounds for one external thumbnailer run.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Wall-clock time before the sandbox is killed.
    pub timeout: Duration,
    /// `RLIMIT_AS` of the thumbnailer.
    pub address_space: u64,
    /// `RLIMIT_CPU`, a backstop below the wall-clock limit's reach.
    pub cpu_seconds: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(15),
            address_space: 2 * 1024 * 1024 * 1024,
            cpu_seconds: 30,
        }
    }
}

/// How external thumbnailers are confined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sandbox {
    /// New user, mount, PID, network, IPC and UTS namespaces; read-only system
    /// files; the input bound read-only; one private writable output folder.
    Bubblewrap(PathBuf),
    /// Direct execution, only for tests of the process bounds.
    #[cfg(test)]
    Unconfined,
}

#[derive(Debug)]
pub enum Outcome {
    Ready(Bitmap),
    /// No thumbnail: unsupported, refused by policy, or failed.
    Unavailable,
    Cancelled,
}

enum Attempt {
    Produced(Vec<u8>),
    /// The thumbnailer ran and failed, timed out or wrote nothing usable.
    Failed,
    /// Nothing ran (no sandbox, no program, no temporary folder).
    NotRun,
    Cancelled,
}

pub struct Generator {
    cache: Option<Cache>,
    thumbnailers: Thumbnailers,
    database: Arc<MimeDatabase>,
    sandbox: Option<Sandbox>,
    limits: Limits,
    facts: fn(&Path) -> MountFacts,
    search_path: Vec<PathBuf>,
}

fn current_facts(path: &Path) -> MountFacts {
    Mounts::read().facts(path)
}

impl Generator {
    /// The user's cache, installed thumbnailers and Bubblewrap when it works
    /// here. Without a working sandbox, only built-in decoding is used.
    pub fn system() -> Self {
        let sandbox = Some(Sandbox::Bubblewrap(BUBBLEWRAP.into()))
            .filter(|_| sandbox_works(Path::new(BUBBLEWRAP)));
        Self {
            cache: Cache::user(),
            thumbnailers: Thumbnailers::load(&Thumbnailers::search_dirs(), &program_path()),
            database: MimeDatabase::system(),
            sandbox,
            limits: Limits::default(),
            facts: current_facts,
            search_path: program_path(),
        }
    }

    pub fn sandboxed(&self) -> bool {
        self.sandbox.is_some()
    }

    /// A thumbnail of at most `flavor`'s side for `path`.
    pub fn thumbnail(
        &self,
        path: &Path,
        flavor: Flavor,
        policy: ThumbnailPolicy,
        cancellation: &Cancellation,
    ) -> Outcome {
        if policy == ThumbnailPolicy::Never || cancellation.is_cancelled() {
            return Outcome::Unavailable;
        }
        if !std::fs::metadata(path).is_ok_and(|meta| meta.is_file()) {
            return Outcome::Unavailable;
        }
        let Some(source) = Source::of(path) else {
            return Outcome::Unavailable;
        };
        if self
            .cache
            .as_ref()
            .is_some_and(|cache| cache.contains(path))
        {
            return Outcome::Unavailable;
        }
        if policy == ThumbnailPolicy::Local && !(self.facts)(path).local() {
            return Outcome::Unavailable;
        }
        if let Some(cache) = &self.cache {
            match cache.lookup(flavor, &source, APPLICATION) {
                Lookup::Found(_, bytes) => {
                    if let Some(bitmap) = decode(bytes, flavor.side(), cancellation) {
                        return Outcome::Ready(bitmap);
                    }
                }
                Lookup::Failed => return Outcome::Unavailable,
                Lookup::Missing => {}
            }
        }
        let mime = self
            .database
            .detect_path(path, canopa_file::mime::DEFAULT_SNIFF_BYTES);
        let mut bitmap = None;
        let mut failed = false;
        if BUILT_IN.contains(&&*mime) {
            bitmap = built_in(path, flavor.side(), cancellation);
            failed = bitmap.is_none();
        }
        if bitmap.is_none()
            && let Some(thumbnailer) = self.thumbnailers.for_types([&*mime])
        {
            match self.run(thumbnailer, path, &source, flavor.side(), cancellation) {
                Attempt::Produced(png) => {
                    bitmap = decode(png, flavor.side(), cancellation);
                    failed = bitmap.is_none();
                }
                Attempt::Failed => failed = true,
                Attempt::NotRun => {}
                Attempt::Cancelled => return Outcome::Cancelled,
            }
        }
        if cancellation.is_cancelled() {
            return Outcome::Cancelled;
        }
        match bitmap {
            Some(bitmap) => {
                if let Some(cache) = &self.cache
                    && let Some(png) = encode(
                        bitmap.width,
                        bitmap.height,
                        &bitmap.rgba,
                        &source,
                        Some(&mime),
                        APPLICATION,
                    )
                {
                    let _ = cache.store(flavor, &source, &png);
                }
                Outcome::Ready(bitmap)
            }
            None => {
                if failed && let Some(cache) = &self.cache {
                    let _ = cache.record_failure(APPLICATION, &source);
                }
                Outcome::Unavailable
            }
        }
    }

    fn run(
        &self,
        thumbnailer: &Thumbnailer,
        path: &Path,
        source: &Source,
        side: u32,
        cancellation: &Cancellation,
    ) -> Attempt {
        let Some(sandbox) = &self.sandbox else {
            return Attempt::NotRun;
        };
        let Some(program) = resolve_program(thumbnailer, &self.search_path) else {
            return Attempt::NotRun;
        };
        let Ok(input) = std::fs::File::open(path) else {
            return Attempt::NotRun;
        };
        if !input.metadata().is_ok_and(|meta| meta.is_file()) {
            return Attempt::NotRun;
        }
        let Ok(output) = tempfile::Builder::new()
            .prefix("canopa-thumbnail-")
            .tempdir_in(private_temporary_dir())
        else {
            return Attempt::NotRun;
        };
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .filter(|extension| {
                extension.len() <= 16 && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
            .map(|extension| format!(".{extension}"))
            .unwrap_or_default();
        let (mut command, inherited) = match sandbox {
            Sandbox::Bubblewrap(bwrap) => {
                // Only programs from the read-only system tree exist inside.
                let Ok(resolved) = std::fs::canonicalize(&program) else {
                    return Attempt::NotRun;
                };
                if !resolved.starts_with("/usr") {
                    return Attempt::NotRun;
                }
                let inside = PathBuf::from(format!("/input/source{extension}"));
                let uri = format!("file:///input/source{extension}");
                let mut arguments =
                    thumbnailer.arguments(&inside, &uri, Path::new("/output/thumbnail.png"), side);
                arguments[0] = program.to_string_lossy().into_owned();
                let mut command = bubblewrap(bwrap);
                command
                    .arg("--ro-bind-fd")
                    .arg(input.as_raw_fd().to_string())
                    .arg(&inside)
                    .arg("--bind")
                    .arg(output.path())
                    .arg("/output")
                    .arg("--")
                    .args(arguments);
                (command, Some(input.as_raw_fd()))
            }
            #[cfg(test)]
            Sandbox::Unconfined => {
                let mut arguments = thumbnailer.arguments(
                    path,
                    &source.uri,
                    &output.path().join("thumbnail.png"),
                    side,
                );
                arguments[0] = program.to_string_lossy().into_owned();
                let mut command = Command::new(&arguments[0]);
                command
                    .args(&arguments[1..])
                    .env_clear()
                    .env("PATH", "/usr/bin:/bin");
                (command, None)
            }
        };
        let _ = source;
        bound(&mut command, self.limits, inherited);
        match run_with_deadline(command, self.limits.timeout, cancellation) {
            Run::Cancelled => return Attempt::Cancelled,
            Run::Failed => return Attempt::Failed,
            Run::Succeeded => {}
        }
        drop(input);
        match read_output(&output.path().join("thumbnail.png")) {
            Some(bytes) => Attempt::Produced(bytes),
            None => Attempt::Failed,
        }
    }
}

/// Where thumbnailer programs are looked up; the sandbox sees only `/usr`.
fn program_path() -> Vec<PathBuf> {
    ["/usr/local/bin", "/usr/bin", "/bin"]
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

fn private_temporary_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_dir())
        .unwrap_or_else(std::env::temp_dir)
}

/// The confinement shared with the portal's print converters (ADR 0076):
/// every namespace, no capabilities, no nested user namespaces, a cleared
/// environment, read-only `/usr`, private `/proc`, `/dev` and a bounded `/tmp`.
fn bubblewrap(bwrap: &Path) -> Command {
    let mut command = Command::new(bwrap);
    command.env_clear().args([
        "--unshare-all",
        "--unshare-user",
        "--disable-userns",
        "--die-with-parent",
        "--as-pid-1",
        "--new-session",
        "--cap-drop",
        "ALL",
        "--clearenv",
        "--setenv",
        "PATH",
        "/usr/bin",
        "--setenv",
        "HOME",
        "/tmp",
        "--setenv",
        "TMPDIR",
        "/tmp",
        "--setenv",
        "XDG_CACHE_HOME",
        "/tmp/.cache",
        "--setenv",
        "LANG",
        "C.UTF-8",
        // GLib-based thumbnailers: no GVfs daemons, no dconf, no session bus.
        "--setenv",
        "GIO_USE_VFS",
        "local",
        "--setenv",
        "GSETTINGS_BACKEND",
        "memory",
        "--ro-bind",
        "/usr",
        "/usr",
    ]);
    for path in ["/bin", "/sbin", "/lib", "/lib64", "/lib32"] {
        match std::fs::read_link(path) {
            Ok(target) => {
                command.arg("--symlink").arg(target).arg(path);
            }
            Err(_) if Path::new(path).is_dir() => {
                command.args(["--ro-bind", path, path]);
            }
            Err(_) => {}
        }
    }
    command.args([
        "--dir",
        "/etc",
        "--ro-bind-try",
        "/etc/fonts",
        "/etc/fonts",
        "--ro-bind-try",
        "/etc/ld.so.cache",
        "/etc/ld.so.cache",
        "--ro-bind-try",
        "/etc/ld.so.conf",
        "/etc/ld.so.conf",
        "--ro-bind-try",
        "/etc/ld.so.conf.d",
        "/etc/ld.so.conf.d",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--size",
        &SANDBOX_TMP_BYTES.to_string(),
        "--tmpfs",
        "/tmp",
        "--dir",
        "/input",
        "--chdir",
        "/tmp",
    ]);
    command
}

/// One probe per process: user namespaces can be disabled by the kernel.
fn sandbox_works(bwrap: &Path) -> bool {
    if !bwrap.is_file() {
        return false;
    }
    let mut command = bubblewrap(bwrap);
    command.args(["--", "/usr/bin/true"]);
    bound(&mut command, Limits::default(), None);
    matches!(
        run_with_deadline(command, Duration::from_secs(5), &Cancellation::default()),
        Run::Succeeded
    )
}

/// Resource limits and descriptor hygiene for the child, applied after fork.
fn bound(command: &mut Command, limits: Limits, inherited: Option<i32>) {
    let parent = rustix::process::getpid();
    // SAFETY: only raw syscalls on captured integers run between fork and exec.
    unsafe {
        command.pre_exec(move || {
            use rustix::process::{Resource, Rlimit, setrlimit};
            rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::KILL))?;
            if rustix::process::getppid() != Some(parent) {
                return Err(rustix::io::Errno::CHILD.into());
            }
            // Rust and GLib open descriptors close-on-exec, but other libraries
            // may not: mark every descriptor above stdio, then pass the input.
            let ceiling = rustix::process::getrlimit(Resource::Nofile)
                .current
                .unwrap_or(65536)
                .min(65536) as i32;
            for fd in 3..ceiling {
                let fd = BorrowedFd::borrow_raw(fd);
                if let Ok(flags) = rustix::io::fcntl_getfd(fd) {
                    let _ = rustix::io::fcntl_setfd(fd, flags | rustix::io::FdFlags::CLOEXEC);
                }
            }
            if let Some(fd) = inherited {
                let fd = BorrowedFd::borrow_raw(fd);
                let flags = rustix::io::fcntl_getfd(fd)?;
                rustix::io::fcntl_setfd(fd, flags & !rustix::io::FdFlags::CLOEXEC)?;
            }
            for (resource, value) in [
                (Resource::Core, 0),
                (Resource::As, limits.address_space),
                (Resource::Cpu, limits.cpu_seconds),
                (Resource::Fsize, MAX_OUTPUT_BYTES as u64),
                (Resource::Nofile, 256),
            ] {
                setrlimit(
                    resource,
                    Rlimit {
                        current: Some(value),
                        maximum: Some(value),
                    },
                )?;
            }
            Ok(())
        });
    }
}

enum Run {
    Succeeded,
    Failed,
    Cancelled,
}

/// Run with a wall-clock deadline. Timeout or cancellation kills the whole
/// process group through [`crate::process::run`].
fn run_with_deadline(command: Command, timeout: Duration, cancellation: &Cancellation) -> Run {
    let stop = Cancellation::default();
    let timed_out = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let watchdog = {
        let (stop, outer) = (stop.clone(), cancellation.clone());
        let (timed_out, finished) = (timed_out.clone(), finished.clone());
        let deadline = Instant::now() + timeout;
        std::thread::Builder::new()
            .name("files-thumbnail-deadline".into())
            .spawn(move || {
                while !finished.load(Ordering::Acquire) {
                    if outer.is_cancelled() {
                        stop.cancel();
                        return;
                    }
                    if Instant::now() >= deadline {
                        timed_out.store(true, Ordering::Release);
                        stop.cancel();
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            })
    };
    let outcome = crate::process::run(command, &stop);
    finished.store(true, Ordering::Release);
    if let Ok(watchdog) = watchdog {
        let _ = watchdog.join();
    }
    match outcome {
        _ if cancellation.is_cancelled() => Run::Cancelled,
        Ok(outcome) if outcome.success && !timed_out.load(Ordering::Acquire) => Run::Succeeded,
        _ => Run::Failed,
    }
}

/// The output must be a regular file the sandbox wrote, never a symlink it
/// planted to make Files read something else.
fn read_output(path: &Path) -> Option<Vec<u8>> {
    use rustix::fs::{Mode, OFlags, open};
    use std::io::Read;
    let fd = open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .ok()?;
    let file = std::fs::File::from(fd);
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_OUTPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= MAX_OUTPUT_BYTES && !bytes.is_empty()).then_some(bytes)
}

fn policy(
    max_encoded_bytes: usize,
    max_dimension: u32,
    allocation: u64,
    side: u32,
    orient: bool,
) -> RasterPolicy {
    RasterPolicy {
        max_encoded_bytes,
        max_dimension,
        max_pixels: allocation / 4,
        decoder_allocation: DecoderAllocation::IncludingOutput(allocation),
        formats: Formats::EnabledRasterCodecs,
        file_access: FileAccess::Follow,
        apply_orientation: orient,
        thumbnail: Some(side),
    }
}

fn budget(encoded_bytes: usize, pixel_bytes: u64) -> RasterBudget {
    RasterBudget {
        file_attempts: 1,
        encoded_bytes,
        pixel_bytes: pixel_bytes as usize,
    }
}

/// Built-in decoding with Quick Look's image limits: 16384 pixels a side and a
/// 256 MiB decoder allocation, from at most 64 MiB of file.
fn built_in(path: &Path, side: u32, cancellation: &Cancellation) -> Option<Bitmap> {
    let cancelled = || cancellation.is_cancelled();
    let allocation = 256 * 1024 * 1024;
    let mut batch = RasterBatch::new(
        policy(MAX_IMAGE_BYTES, 16_384, allocation, side, true),
        budget(MAX_IMAGE_BYTES, allocation),
        &cancelled,
    );
    batch.load(path).map(bitmap)
}

/// Decode a cached or produced thumbnail, scaled to fit `side`.
fn decode(bytes: Vec<u8>, side: u32, cancellation: &Cancellation) -> Option<Bitmap> {
    let cancelled = || cancellation.is_cancelled();
    let allocation = 64 * 1024 * 1024;
    let mut batch = RasterBatch::new(
        policy(
            MAX_THUMBNAIL_BYTES.max(MAX_OUTPUT_BYTES),
            4096,
            allocation,
            side,
            false,
        ),
        budget(MAX_OUTPUT_BYTES, allocation),
        &cancelled,
    );
    batch.decode(bytes).map(bitmap)
}

fn bitmap(raster: canopa_assets::Raster) -> Bitmap {
    Bitmap {
        width: raster.width,
        height: raster.height,
        rgba: raster.pixels.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn local(_: &Path) -> MountFacts {
        MountFacts {
            placement: crate::mounts::Placement::Local,
            unix_permissions: true,
            noexec: false,
        }
    }

    fn removable(_: &Path) -> MountFacts {
        MountFacts {
            placement: crate::mounts::Placement::Removable,
            ..local(Path::new("/"))
        }
    }

    struct Fixture {
        root: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                root: tempfile::tempdir().unwrap(),
            }
        }
        fn cache(&self) -> Cache {
            Cache::new(self.root.path().join("cache/thumbnails"))
        }
        fn generator(
            &self,
            thumbnailer: Option<&str>,
            sandbox: Option<Sandbox>,
            timeout: Duration,
        ) -> Generator {
            let dir = self.root.path().join("data/thumbnailers");
            std::fs::create_dir_all(&dir).unwrap();
            if let Some(exec) = thumbnailer {
                std::fs::write(
                    dir.join("fake.thumbnailer"),
                    format!("[Thumbnailer Entry]\nExec={exec}\nMimeType=video/mp4;\n"),
                )
                .unwrap();
            }
            let mime = self.root.path().join("mime");
            std::fs::create_dir_all(&mime).unwrap();
            std::fs::write(
                mime.join("globs2"),
                "50:video/mp4:*.mp4\n50:image/png:*.png\n",
            )
            .unwrap();
            let search: Vec<PathBuf> = vec![self.root.path().join("bin"), "/usr/bin".into()];
            Generator {
                cache: Some(self.cache()),
                thumbnailers: Thumbnailers::load(&[dir], &search),
                database: Arc::new(MimeDatabase::load(&[mime])),
                sandbox,
                limits: Limits {
                    timeout,
                    ..Limits::default()
                },
                facts: local,
                search_path: search,
            }
        }
        fn png(&self, name: &str, width: u32, height: u32, pixel: [u8; 4]) -> PathBuf {
            let path = self.root.path().join(name);
            image::RgbaImage::from_pixel(width, height, image::Rgba(pixel))
                .save_with_format(&path, image::ImageFormat::Png)
                .unwrap();
            path
        }
    }

    fn ready(outcome: Outcome) -> Bitmap {
        match outcome {
            Outcome::Ready(bitmap) => bitmap,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn built_in_images_are_scaled_cached_and_reused() {
        let fixture = Fixture::new();
        let generator = fixture.generator(None, None, Duration::from_secs(5));
        let photo = fixture.png("photo.png", 600, 300, [200, 10, 10, 255]);
        let cancel = Cancellation::default();
        let first =
            ready(generator.thumbnail(&photo, Flavor::Normal, ThumbnailPolicy::Local, &cancel));
        assert_eq!((first.width, first.height), (128, 64));
        let source = Source::of(&photo).unwrap();
        let stored = fixture.cache().path(Flavor::Normal, &source);
        let metadata =
            canopa_assets::thumbnails::Metadata::read(&std::fs::read(&stored).unwrap()).unwrap();
        assert!(metadata.describes(&source));
        assert_eq!(metadata.mime.as_deref(), Some("image/png"));
        assert_eq!(metadata.software.as_deref(), Some(APPLICATION));
        // A valid cached thumbnail wins over decoding the file again.
        let blue = encode(4, 4, &[0, 0, 255, 255].repeat(16), &source, None, "other").unwrap();
        std::fs::write(&stored, blue).unwrap();
        let cached =
            ready(generator.thumbnail(&photo, Flavor::Normal, ThumbnailPolicy::Local, &cancel));
        assert_eq!(&cached.rgba[..4], &[0, 0, 255, 255]);
        // Policy: never, and local-only on removable media.
        assert!(matches!(
            generator.thumbnail(&photo, Flavor::Normal, ThumbnailPolicy::Never, &cancel),
            Outcome::Unavailable
        ));
        let stick = Generator {
            facts: removable,
            ..fixture.generator(None, None, Duration::from_secs(5))
        };
        assert!(matches!(
            stick.thumbnail(&photo, Flavor::Normal, ThumbnailPolicy::Local, &cancel),
            Outcome::Unavailable
        ));
        assert!(matches!(
            stick.thumbnail(&photo, Flavor::Normal, ThumbnailPolicy::Always, &cancel),
            Outcome::Ready(_)
        ));
        // Files inside the cache are never thumbnailed.
        assert!(matches!(
            generator.thumbnail(&stored, Flavor::Normal, ThumbnailPolicy::Always, &cancel),
            Outcome::Unavailable
        ));
    }

    #[test]
    fn broken_images_get_a_failure_record_until_they_change() {
        let fixture = Fixture::new();
        let generator = fixture.generator(None, None, Duration::from_secs(5));
        let broken = fixture.root.path().join("broken.png");
        std::fs::write(&broken, b"\x89PNG\r\n\x1a\nnot really").unwrap();
        let cancel = Cancellation::default();
        assert!(matches!(
            generator.thumbnail(&broken, Flavor::Normal, ThumbnailPolicy::Local, &cancel),
            Outcome::Unavailable
        ));
        let source = Source::of(&broken).unwrap();
        assert_eq!(
            fixture.cache().lookup(Flavor::Normal, &source, APPLICATION),
            Lookup::Failed
        );
        // An unsupported type is not a failure.
        let text = fixture.root.path().join("notes.txt");
        std::fs::write(&text, "hello").unwrap();
        assert!(matches!(
            generator.thumbnail(&text, Flavor::Normal, ThumbnailPolicy::Local, &cancel),
            Outcome::Unavailable
        ));
        assert_eq!(
            fixture
                .cache()
                .lookup(Flavor::Normal, &Source::of(&text).unwrap(), APPLICATION),
            Lookup::Missing
        );
    }

    fn script(fixture: &Fixture, name: &str, body: &str) {
        let bin = fixture.root.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let path = bin.join(name);
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn a_hanging_thumbnailer_is_killed_at_its_deadline_and_recorded_as_failed() {
        let fixture = Fixture::new();
        let marker = fixture.root.path().join("still-running");
        script(
            &fixture,
            "slow-thumbnailer",
            &format!("#!/bin/sh\nsleep 30\ntouch '{}'\n", marker.display()),
        );
        let generator = fixture.generator(
            Some("slow-thumbnailer %i %o %s"),
            Some(Sandbox::Unconfined),
            Duration::from_millis(400),
        );
        let video = fixture.root.path().join("clip.mp4");
        std::fs::write(&video, b"not a video").unwrap();
        let started = Instant::now();
        let outcome = generator.thumbnail(
            &video,
            Flavor::Large,
            ThumbnailPolicy::Local,
            &Cancellation::default(),
        );
        assert!(matches!(outcome, Outcome::Unavailable), "{outcome:?}");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(
            fixture
                .cache()
                .lookup(Flavor::Large, &Source::of(&video).unwrap(), APPLICATION),
            Lookup::Failed
        );
        std::thread::sleep(Duration::from_millis(200));
        assert!(!marker.exists());
    }

    #[test]
    fn cancelling_stops_a_thumbnailer_without_a_failure_record() {
        let fixture = Fixture::new();
        script(&fixture, "slow-thumbnailer", "#!/bin/sh\nsleep 30\n");
        let generator = fixture.generator(
            Some("slow-thumbnailer %i %o"),
            Some(Sandbox::Unconfined),
            Duration::from_secs(20),
        );
        let video = fixture.root.path().join("clip.mp4");
        std::fs::write(&video, b"x").unwrap();
        let cancel = Cancellation::default();
        let trigger = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            trigger.cancel();
        });
        let started = Instant::now();
        assert!(matches!(
            generator.thumbnail(&video, Flavor::Normal, ThumbnailPolicy::Local, &cancel),
            Outcome::Cancelled
        ));
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(
            fixture
                .cache()
                .lookup(Flavor::Normal, &Source::of(&video).unwrap(), APPLICATION),
            Lookup::Missing
        );
    }

    #[test]
    fn external_output_is_validated_scaled_and_stored() {
        let fixture = Fixture::new();
        let produced = fixture.png("produced.png", 1000, 500, [5, 6, 7, 255]);
        script(
            &fixture,
            "copy-thumbnailer",
            &format!("#!/bin/sh\ncp '{}' \"$2\"\n", produced.display()),
        );
        script(
            &fixture,
            "link-thumbnailer",
            "#!/bin/sh\nln -s /etc/passwd \"$2\"\n",
        );
        let video = fixture.root.path().join("clip.mp4");
        std::fs::write(&video, b"x").unwrap();
        let generator = fixture.generator(
            Some("copy-thumbnailer %i %o"),
            Some(Sandbox::Unconfined),
            Duration::from_secs(10),
        );
        let bitmap = ready(generator.thumbnail(
            &video,
            Flavor::Large,
            ThumbnailPolicy::Local,
            &Cancellation::default(),
        ));
        assert_eq!((bitmap.width, bitmap.height), (256, 128));
        let source = Source::of(&video).unwrap();
        assert!(matches!(
            fixture.cache().lookup(Flavor::Large, &source, APPLICATION),
            Lookup::Found(Flavor::Large, _)
        ));
        let other = fixture.root.path().join("other.mp4");
        std::fs::write(&other, b"y").unwrap();
        let planted = fixture.generator(
            Some("link-thumbnailer %i %o"),
            Some(Sandbox::Unconfined),
            Duration::from_secs(10),
        );
        assert!(matches!(
            planted.thumbnail(
                &other,
                Flavor::Large,
                ThumbnailPolicy::Local,
                &Cancellation::default()
            ),
            Outcome::Unavailable
        ));
    }

    /// The real sandbox, when this machine allows unprivileged namespaces.
    fn bubblewrap_or_skip() -> Option<Sandbox> {
        if sandbox_works(Path::new(BUBBLEWRAP)) {
            Some(Sandbox::Bubblewrap(BUBBLEWRAP.into()))
        } else {
            eprintln!("skipping: Bubblewrap is unavailable here");
            None
        }
    }

    #[test]
    fn the_sandbox_sees_a_read_only_input_and_nothing_of_the_user() {
        let Some(sandbox) = bubblewrap_or_skip() else {
            return;
        };
        let fixture = Fixture::new();
        let input = fixture.png("clip.mp4", 300, 300, [1, 200, 3, 255]);
        let home = fixture.root.path().display().to_string();
        // The thumbnail is the input itself, copied only when every probe of
        // the confinement holds.
        let exec = format!(
            "sh -c 'test ! -e \"$2\" || exit 11; echo x >> \"$0\" 2>/dev/null && exit 12; \
             test -z \"$DBUS_SESSION_BUS_ADDRESS$WAYLAND_DISPLAY\" || exit 13; \
             test \"$(cat /proc/net/dev | wc -l)\" -le 3 || exit 14; cp \"$0\" \"$1\"' %i %o {home}"
        );
        let generator =
            fixture.generator(Some(&exec), Some(sandbox.clone()), Duration::from_secs(10));
        let bitmap = ready(generator.thumbnail(
            &input,
            Flavor::Normal,
            ThumbnailPolicy::Local,
            &Cancellation::default(),
        ));
        assert_eq!((bitmap.width, bitmap.height), (128, 128));
        assert_eq!(&bitmap.rgba[..4], &[1, 200, 3, 255]);
        let hanging = fixture.root.path().join("hang.mp4");
        std::fs::write(&hanging, b"x").unwrap();
        let generator = fixture.generator(
            Some("sh -c 'sleep 30' %o"),
            Some(sandbox),
            Duration::from_millis(500),
        );
        let started = Instant::now();
        assert!(matches!(
            generator.thumbnail(
                &hanging,
                Flavor::Normal,
                ThumbnailPolicy::Local,
                &Cancellation::default()
            ),
            Outcome::Unavailable
        ));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    /// Hands-on: the installed glycin thumbnailer through the real sandbox.
    /// `cargo test -p canopa-files --lib installed_thumbnailers -- --ignored`
    #[test]
    #[ignore = "needs installed thumbnailers and unprivileged namespaces"]
    fn installed_thumbnailers_render_through_the_sandbox() {
        let sandbox = bubblewrap_or_skip().expect("Bubblewrap works here");
        let fixture = Fixture::new();
        let generator = Generator {
            thumbnailers: Thumbnailers::load(&Thumbnailers::search_dirs(), &program_path()),
            database: MimeDatabase::system(),
            sandbox: Some(sandbox),
            search_path: program_path(),
            ..fixture.generator(None, None, Duration::from_secs(15))
        };
        // A PNG named as TIFF reaches glycin, which reads the content.
        let photo = fixture.png("photo.tiff", 640, 480, [30, 90, 200, 255]);
        let bitmap = ready(generator.thumbnail(
            &photo,
            Flavor::Normal,
            ThumbnailPolicy::Always,
            &Cancellation::default(),
        ));
        assert_eq!((bitmap.width, bitmap.height), (128, 96));
    }
}
