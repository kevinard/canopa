//! Selected-user thumbnails, independent of account eligibility and authentication.
use anyhow::{Result, ensure};
use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
};
use image::{
    RgbaImage,
    imageops::{self, FilterType},
};
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    process::Command,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

const SIDE: u32 = 128;
const OUTPUT: usize = SIDE as usize * SIDE as usize * 4;
type Thumbnail = Option<Arc<RgbaImage>>;

#[derive(Clone, Debug)]
pub struct Policy {
    pub enabled: bool,
    pub directory: PathBuf,
    pub users: BTreeMap<String, PathBuf>,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            enabled: true,
            directory: "/var/lib/AccountsService/icons".into(),
            users: BTreeMap::new(),
        }
    }
}
impl Policy {
    pub fn paths(&self, username: &str, icon: Option<&std::path::Path>) -> Vec<PathBuf> {
        if !self.enabled || !crate::config::valid_user(username) {
            return Vec::new();
        }
        let mut paths = Vec::new();
        let directory = self.directory.join(username);
        for path in self
            .users
            .get(username)
            .map(PathBuf::as_path)
            .into_iter()
            .chain(icon)
            .chain(std::iter::once(directory.as_path()))
        {
            if valid_path(path) && !paths.iter().any(|p| p == path) {
                paths.push(path.to_path_buf());
            }
        }
        paths
    }
}
pub(crate) fn valid_path(path: &std::path::Path) -> bool {
    path.is_absolute()
        && path
            .to_str()
            .is_some_and(|s| s.len() <= 4096 && !s.chars().any(char::is_control))
}
pub fn initials(name: &str) -> String {
    let words = name
        .split_whitespace()
        .filter_map(|s| s.chars().find(|c| c.is_alphanumeric()))
        .take(2)
        .collect::<String>();
    let letters = if words.chars().count() > 1 {
        words
    } else {
        name.chars()
            .filter(|c| c.is_alphanumeric())
            .take(2)
            .collect()
    };
    let letters = letters.to_uppercase().chars().take(2).collect::<String>();
    if letters.is_empty() {
        "?".into()
    } else {
        letters
    }
}

/// Only the fresh hidden helper calls the decoder; paths are literal argv, never commands.
pub fn helper(paths: &[PathBuf]) -> Result<Option<RgbaImage>> {
    canopa_session_ui::secret::protect_process()?;
    prepare(paths)
}
/// Preview reads only the explicitly configured selected-user asset, without discovery.
pub fn preview(policy: &Policy, username: &str) -> Result<Option<RgbaImage>> {
    match policy.users.get(username).filter(|_| policy.enabled) {
        Some(path) => prepare(std::slice::from_ref(path)),
        None => Ok(None),
    }
}
fn prepare(paths: &[PathBuf]) -> Result<Option<RgbaImage>> {
    ensure!(
        paths.len() <= 3 && paths.iter().all(|p| valid_path(p)),
        "invalid avatar sources"
    );
    let mut batch = RasterBatch::new(
        RasterPolicy {
            max_encoded_bytes: 2 * 1024 * 1024,
            max_dimension: 2048,
            max_pixels: 4_194_304,
            decoder_allocation: DecoderAllocation::IncludingOutput(32 * 1024 * 1024),
            formats: Formats::EnabledRasterCodecs,
            file_access: FileAccess::Follow,
            apply_orientation: true,
            thumbnail: Some(256),
        },
        RasterBudget {
            file_attempts: 3,
            encoded_bytes: 6 * 1024 * 1024,
            pixel_bytes: 48 * 1024 * 1024,
        },
        &|| false,
    );
    for path in paths {
        if let Some(raster) = batch.load(path)
            && let Some(image) = RgbaImage::from_raw(raster.width, raster.height, raster.pixels)
        {
            let side = image.width().min(image.height());
            let square = imageops::crop_imm(
                &image,
                (image.width() - side) / 2,
                (image.height() - side) / 2,
                side,
                side,
            );
            return Ok(Some(imageops::resize(
                &square.to_image(),
                SIDE,
                SIDE,
                FilterType::Triangle,
            )));
        }
    }
    Ok(None)
}
fn load(paths: &[PathBuf], cancelled: &dyn Fn() -> bool) -> Thumbnail {
    if paths.is_empty() || cancelled() {
        return None;
    }
    let mut command = Command::new(std::env::current_exe().ok()?);
    command.arg("internal-avatar").arg("--").args(paths);
    let bytes = crate::helper::run(command, cancelled, OUTPUT).ok()?;
    RgbaImage::from_raw(SIDE, SIDE, bytes.to_vec()).map(Arc::new)
}

struct Shared {
    pending: Mutex<Option<(u64, Vec<PathBuf>)>>,
    wake: Condvar,
    generation: AtomicU64,
    stopped: AtomicBool,
}
pub struct Worker {
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(notify: impl Fn(u64, Thumbnail) + Send + 'static) -> Self {
        Self::with_loader(notify, load)
    }
    fn with_loader(
        notify: impl Fn(u64, Thumbnail) + Send + 'static,
        loader: impl Fn(&[PathBuf], &dyn Fn() -> bool) -> Thumbnail + Send + 'static,
    ) -> Self {
        let shared = Arc::new(Shared {
            pending: Mutex::new(None),
            wake: Condvar::new(),
            generation: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
        });
        let state = shared.clone();
        let thread = std::thread::spawn(move || {
            let mut cache: VecDeque<(Vec<PathBuf>, Thumbnail)> = VecDeque::new();
            loop {
                let (generation, paths) = {
                    let mut pending = state.pending.lock().unwrap();
                    while pending.is_none() && !state.stopped.load(Ordering::Acquire) {
                        pending = state.wake.wait(pending).unwrap();
                    }
                    if state.stopped.load(Ordering::Acquire) {
                        break;
                    }
                    pending.take().unwrap()
                };
                let cancelled = || {
                    state.stopped.load(Ordering::Acquire)
                        || state.generation.load(Ordering::Acquire) != generation
                };
                let image = if let Some(index) = cache.iter().position(|(key, _)| key == &paths) {
                    let entry = cache.remove(index).unwrap();
                    let image = entry.1.clone();
                    cache.push_front(entry);
                    image
                } else {
                    let image = loader(&paths, &cancelled);
                    if !cancelled() {
                        cache.push_front((paths, image.clone()));
                        cache.truncate(8);
                    }
                    image
                };
                if !cancelled() {
                    notify(generation, image);
                }
            }
        });
        Self {
            shared,
            thread: Some(thread),
        }
    }
    pub fn request(&self, paths: Vec<PathBuf>) -> u64 {
        let mut pending = self.shared.pending.lock().unwrap();
        let generation = self.shared.generation.fetch_add(1, Ordering::AcqRel) + 1;
        *pending = Some((generation, paths));
        self.shared.wake.notify_one();
        generation
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _pending = self.shared.pending.lock().unwrap();
        self.shared.stopped.store(true, Ordering::Release);
        self.shared.wake.notify_one();
        drop(_pending);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn pending_selection_cancels_stale_work_and_reuses_a_bounded_thumbnail_cache() {
        let calls = Arc::new(AtomicU64::new(0));
        let counter = calls.clone();
        let (began, started) = std::sync::mpsc::channel();
        let (notify, results) = std::sync::mpsc::channel();
        let worker = Worker::with_loader(
            move |id, image| {
                notify.send((id, image)).unwrap();
            },
            move |paths, cancelled| {
                counter.fetch_add(1, Ordering::Relaxed);
                if paths == [PathBuf::from("slow")] {
                    began.send(()).unwrap();
                    let deadline = Instant::now() + Duration::from_secs(2);
                    while !cancelled() {
                        assert!(Instant::now() < deadline);
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    return None;
                }
                Some(Arc::new(RgbaImage::new(1, 1)))
            },
        );
        worker.request(vec!["slow".into()]);
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        let current = worker.request(vec!["current".into()]);
        let (generation, image) = results.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(generation, current);
        let image = image.unwrap();
        let again = worker.request(vec!["current".into()]);
        let (generation, cached) = results.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(generation, again);
        assert!(Arc::ptr_eq(&image, &cached.unwrap()));
        assert_eq!(calls.load(Ordering::Relaxed), 2);
        for n in 0..8 {
            worker.request(vec![format!("other{n}").into()]);
            results.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        worker.request(vec!["current".into()]);
        results.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 11);
        worker.request(vec!["slow".into()]);
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        let stop = Instant::now();
        drop(worker);
        assert!(stop.elapsed() < Duration::from_millis(500));
        assert!(results.try_recv().is_err());
    }
    #[test]
    fn sources_are_ordered_local_paths_and_initials_are_bounded() {
        let mut policy = Policy {
            directory: "/faces".into(),
            ..Policy::default()
        };
        policy.users.insert("alice".into(), "/override.png".into());
        assert_eq!(
            policy.paths("alice", Some(std::path::Path::new("/service"))),
            [
                PathBuf::from("/override.png"),
                "/service".into(),
                "/faces/alice".into()
            ]
        );
        assert_eq!(
            policy
                .paths(
                    "alice",
                    Some(std::path::Path::new("https://example.org/pic"))
                )
                .len(),
            2
        );
        assert!(policy.paths("../alice", None).is_empty());
        policy.enabled = false;
        assert!(policy.paths("alice", None).is_empty());
        assert_eq!(initials("Alice Example"), "AE");
        assert_eq!(initials("alice"), "AL");
        assert_eq!(initials(""), "?");
    }
}
