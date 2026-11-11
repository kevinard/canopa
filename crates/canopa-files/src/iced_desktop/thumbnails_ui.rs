//! Thumbnails for the entries a window shows. After each update the visible
//! rows of every active tab are compared with what is prepared; missing ones
//! queue for a small worker pool (`crate::thumbnails`). Work for a folder no
//! tab shows any more is cancelled, which kills its sandboxed thumbnailer.
//! Views only clone prepared handles, keyed by path and modification time.
use super::*;
use crate::{
    browse::Entry,
    interface::{ThumbnailPolicy, ThumbnailSettings},
    thumbnails::{Generator, Outcome},
};
use canopa_assets::thumbnails::Flavor;
use iced::widget::image;
use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    hash::{Hash, Hasher},
    sync::OnceLock,
    time::SystemTime,
};

/// Concurrent thumbnail workers, each running at most one sandbox.
const WORKERS: usize = 3;
const MAX_HANDLES: usize = 4096;
const MAX_QUEUE: usize = 512;

static GENERATOR: OnceLock<Arc<Generator>> = OnceLock::new();

/// Reads the installed thumbnailers and probes the sandbox once per process.
fn generator() -> Arc<Generator> {
    GENERATOR
        .get_or_init(|| Arc::new(Generator::system()))
        .clone()
}

struct Job {
    generation: u64,
    folder: PathBuf,
    cancellation: Cancellation,
}

#[derive(Default)]
pub(super) struct Thumbnails {
    /// `None`: tried, and this version of the file has no thumbnail.
    handles: HashMap<PathBuf, (Option<SystemTime>, Option<image::Handle>)>,
    running: HashMap<PathBuf, Job>,
    queue: VecDeque<(PathBuf, Option<SystemTime>, PathBuf)>,
    settings: Option<ThumbnailSettings>,
    fingerprint: Option<u64>,
    generation: u64,
}

#[derive(Debug)]
pub(super) struct Done {
    generation: u64,
    path: PathBuf,
    modified: Option<SystemTime>,
    handle: Option<image::Handle>,
    cancelled: bool,
}

impl Thumbnails {
    /// A prepared thumbnail for this exact version of the entry.
    pub fn handle(&self, entry: &Entry) -> Option<&image::Handle> {
        self.handles
            .get(&entry.path)
            .filter(|(modified, _)| *modified == entry.modified)
            .and_then(|(_, handle)| handle.as_ref())
    }

    fn known(&self, entry: &Entry) -> bool {
        self.handles
            .get(&entry.path)
            .is_some_and(|(modified, _)| *modified == entry.modified)
    }

    fn cancel_all(&mut self) {
        for job in self.running.values() {
            job.cancellation.cancel();
        }
        self.running.clear();
        self.queue.clear();
    }

    #[cfg(test)]
    pub fn pending(&self) -> (usize, usize) {
        (self.running.len(), self.queue.len())
    }
}

impl Drop for Thumbnails {
    fn drop(&mut self) {
        for job in self.running.values() {
            job.cancellation.cancel();
        }
    }
}

impl Desktop {
    /// Queue thumbnails for visible rows. An unchanged layout costs O(windows).
    pub(super) fn sync_thumbnails(&mut self) -> Task<Message> {
        let settings = self.configuration.settings.thumbnails;
        let state = &mut self.icons.thumbnails;
        if state.settings != Some(settings) {
            state.cancel_all();
            state.handles.clear();
            state.settings = Some(settings);
            state.fingerprint = None;
            state.generation += 1;
        }
        if settings.show == ThumbnailPolicy::Never {
            return Task::none();
        }
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for (id, window) in &self.windows {
            let Some(tab) = window.tabs.get(window.active) else {
                continue;
            };
            (id, tab.id, tab.revision, tab.list_epoch, tab.loading).hash(&mut hasher);
            (tab.viewport_height.to_bits(), tab.viewport_width.to_bits()).hash(&mut hasher);
            if let Some(browser) = &tab.browser {
                let entries = &browser.directory().entries;
                (entries.as_ptr() as usize, entries.len()).hash(&mut hasher);
                if let Some(search) = browser.search() {
                    (search.entries.as_ptr() as usize, search.entries.len()).hash(&mut hasher);
                }
                (browser.scroll().to_bits(), browser.show_hidden()).hash(&mut hasher);
                browser.path().hash(&mut hasher);
                format!("{:?}", browser.view().mode).hash(&mut hasher);
            }
        }
        let fingerprint = hasher.finish();
        if self.icons.thumbnails.fingerprint == Some(fingerprint) {
            return Task::none();
        }
        self.icons.thumbnails.fingerprint = Some(fingerprint);
        let mut folders = BTreeSet::new();
        let mut wanted = Vec::new();
        for window in self.windows.values() {
            let Some(tab) = window.tabs.get(window.active) else {
                continue;
            };
            let Some(browser) = tab.browser.as_ref().filter(|_| !tab.loading) else {
                continue;
            };
            folders.insert(browser.path().to_owned());
            let entries = browser.visible_entries();
            let geometry = listing_ui::geometry(self, tab);
            let rows = entries.len().div_ceil(geometry.columns);
            let range =
                view::visible_rows(rows, browser.scroll(), tab.viewport_height, geometry.height);
            let start = range.start * geometry.columns;
            let end = (range.end * geometry.columns).min(entries.len());
            for entry in entries.get(start..end).into_iter().flatten() {
                if !entry.is_directory() && !self.icons.thumbnails.known(entry) {
                    wanted.push((
                        entry.path.clone(),
                        entry.modified,
                        browser.path().to_owned(),
                    ));
                }
            }
        }
        let state = &mut self.icons.thumbnails;
        // Leaving a folder cancels its work; scrolling within it does not.
        state.running.retain(|_, job| {
            let keep = folders.contains(&job.folder);
            if !keep {
                job.cancellation.cancel();
            }
            keep
        });
        state.queue = wanted
            .into_iter()
            .filter(|(path, ..)| !state.running.contains_key(path))
            .take(MAX_QUEUE)
            .collect();
        self.start_thumbnails()
    }

    fn start_thumbnails(&mut self) -> Task<Message> {
        let Some(settings) = self.icons.thumbnails.settings else {
            return Task::none();
        };
        let flavor = Flavor::for_pixels((settings.size.pixels() * 2.0) as u32);
        let state = &mut self.icons.thumbnails;
        let mut tasks = Vec::new();
        while state.running.len() < WORKERS {
            let Some((path, modified, folder)) = state.queue.pop_front() else {
                break;
            };
            let cancellation = Cancellation::default();
            let generation = state.generation;
            state.running.insert(
                path.clone(),
                Job {
                    generation,
                    folder,
                    cancellation: cancellation.clone(),
                },
            );
            tasks.push(Task::perform(
                async move {
                    let (send, receive) = iced::futures::channel::oneshot::channel();
                    let worker_path = path.clone();
                    let worker_cancel = cancellation.clone();
                    let _ = std::thread::Builder::new()
                        .name("files-thumbnail".into())
                        .spawn(move || {
                            let outcome = generator().thumbnail(
                                &worker_path,
                                flavor,
                                settings.show,
                                &worker_cancel,
                            );
                            let _ = send.send(outcome);
                        });
                    let outcome = receive.await.unwrap_or(Outcome::Cancelled);
                    let (handle, cancelled) = match outcome {
                        Outcome::Ready(bitmap) => (
                            Some(image::Handle::from_rgba(
                                bitmap.width,
                                bitmap.height,
                                bitmap.rgba.to_vec(),
                            )),
                            false,
                        ),
                        Outcome::Unavailable => (None, false),
                        Outcome::Cancelled => (None, true),
                    };
                    Arc::new(Done {
                        generation,
                        path,
                        modified,
                        handle,
                        cancelled: cancelled || cancellation.is_cancelled(),
                    })
                },
                Message::ThumbnailReady,
            ));
        }
        Task::batch(tasks)
    }

    pub(super) fn thumbnail_done(&mut self, done: &Done) -> Task<Message> {
        let state = &mut self.icons.thumbnails;
        if state
            .running
            .get(&done.path)
            .is_some_and(|job| job.generation == done.generation)
        {
            state.running.remove(&done.path);
        }
        if done.generation == state.generation && !done.cancelled {
            if state.handles.len() >= MAX_HANDLES {
                state.handles.clear();
            }
            state
                .handles
                .insert(done.path.clone(), (done.modified, done.handle.clone()));
        }
        self.start_thumbnails()
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{desktop, loaded};
    use super::*;

    #[test]
    fn only_visible_files_queue_and_leaving_the_folder_cancels_their_work() {
        let root = tempfile::tempdir().unwrap();
        let photos = root.path().join("photos");
        std::fs::create_dir(&photos).unwrap();
        for index in 0..400 {
            std::fs::write(photos.join(format!("{index:03}.png")), b"x").unwrap();
        }
        std::fs::create_dir(photos.join("album")).unwrap();
        let (mut desktop, window) = desktop(root.path(), vec![photos.clone()]);
        let tab = desktop.active(window).unwrap().id;
        loaded(&mut desktop, window, tab, 1, &photos);
        let current = desktop.active_mut(window).unwrap();
        current.viewport_height = 400.0;
        current.viewport_width = 800.0;
        let _ = current
            .browser
            .as_mut()
            .unwrap()
            .set_view(crate::preferences::ViewSettings {
                mode: crate::preferences::ViewMode::Icons,
                ..Default::default()
            });
        drop(desktop.sync_thumbnails());
        let (running, queued) = desktop.icons.thumbnails.pending();
        assert_eq!(running, WORKERS);
        assert!(running + queued < 200, "{running} + {queued} of 401");
        assert!(
            !desktop
                .icons
                .thumbnails
                .running
                .contains_key(&photos.join("album"))
        );
        let cancellations: Vec<Cancellation> = desktop
            .icons
            .thumbnails
            .running
            .values()
            .map(|job| job.cancellation.clone())
            .collect();
        // An unchanged layout does not requeue.
        drop(desktop.sync_thumbnails());
        assert_eq!(desktop.icons.thumbnails.pending().0, WORKERS);
        // Navigating away (update syncs thumbnails at its end) cancels the work.
        let _task = desktop.update(Message::Go(window, root.path().to_owned()));
        let revision = desktop.active(window).unwrap().revision;
        loaded(&mut desktop, window, tab, revision, root.path());
        assert_eq!(desktop.active(window).unwrap().path(), root.path());
        assert!(cancellations.iter().all(Cancellation::is_cancelled));
        assert_eq!(desktop.icons.thumbnails.pending(), (0, 0));
    }

    #[test]
    fn results_install_for_their_version_and_never_policy_stops_everything() {
        let root = tempfile::tempdir().unwrap();
        let photo = root.path().join("a.png");
        std::fs::write(&photo, b"x").unwrap();
        let (mut desktop, window) = desktop(root.path(), vec![root.path().to_owned()]);
        let tab = desktop.active(window).unwrap().id;
        loaded(&mut desktop, window, tab, 1, root.path());
        drop(desktop.sync_thumbnails());
        let entry = crate::browse::Entry::read(photo.clone()).unwrap();
        let generation = desktop.icons.thumbnails.generation;
        let handle = image::Handle::from_rgba(1, 1, vec![1, 2, 3, 255]);
        drop(desktop.thumbnail_done(&Done {
            generation,
            path: photo.clone(),
            modified: entry.modified,
            handle: Some(handle.clone()),
            cancelled: false,
        }));
        assert_eq!(desktop.icons.thumbnails.handle(&entry), Some(&handle));
        let mut stale = entry.clone();
        stale.modified = Some(SystemTime::UNIX_EPOCH);
        assert!(desktop.icons.thumbnails.handle(&stale).is_none());
        desktop.configuration.settings.thumbnails.show = ThumbnailPolicy::Never;
        drop(desktop.sync_thumbnails());
        assert!(desktop.icons.thumbnails.handle(&entry).is_none());
        assert_eq!(desktop.icons.thumbnails.pending(), (0, 0));
    }
}
