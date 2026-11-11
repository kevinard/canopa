//! Displayed folders follow outside changes. Watch rescans keep the tab's
//! navigation revision and list epoch, so selection, focus, scroll position and
//! in-flight operation origins stay current; only vanished entries are pruned.
use super::*;
use crate::folder_watch::{FolderWatch, Timing};
use iced::futures::{StreamExt, channel::mpsc};
use std::{collections::BTreeSet, sync::Mutex};

#[derive(Clone)]
struct Notices(Arc<Mutex<Option<mpsc::Receiver<()>>>>);
impl PartialEq for Notices {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Notices {}
impl std::hash::Hash for Notices {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&Arc::as_ptr(&self.0), state);
    }
}

#[derive(Default)]
pub(super) struct State {
    watch: Option<FolderWatch>,
    notices: Option<Notices>,
    watched: BTreeSet<PathBuf>,
}

impl State {
    /// Without inotify the browser keeps working with manual Refresh.
    pub fn start() -> Self {
        match FolderWatch::start(Timing::default()) {
            Ok(watch) => Self {
                notices: watch
                    .take_notices()
                    .map(|receiver| Notices(Arc::new(Mutex::new(Some(receiver))))),
                watch: Some(watch),
                watched: BTreeSet::new(),
            },
            Err(error) => {
                eprintln!(
                    "canopa-files: automatic folder refresh is unavailable ({error}); use Refresh (F5)"
                );
                Self::default()
            }
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let Some(notices) = &self.notices else {
            return Subscription::none();
        };
        Subscription::run_with(notices.clone(), |key| {
            let receiver = key.0.lock().unwrap_or_else(|e| e.into_inner()).take();
            iced::futures::stream::unfold(receiver, |receiver| async move {
                let mut receiver = receiver?;
                receiver.next().await?;
                Some((Message::FoldersChanged, Some(receiver)))
            })
        })
    }
}

/// Every folder a Browser Window displays: each tab's folder, including one
/// still loading, and the ancestor columns of the active tab.
pub(super) fn displayed(windows: &BTreeMap<window::Id, BrowserWindow>) -> BTreeSet<PathBuf> {
    let mut folders = BTreeSet::new();
    for window in windows.values() {
        for (index, tab) in window.tabs.iter().enumerate() {
            if let Some(browser) = &tab.browser {
                folders.insert(browser.path().to_owned());
            }
            if tab.loading && tab.target.is_absolute() {
                folders.insert(tab.target.clone());
            }
            if index == window.active {
                folders.extend(tab.columns.keys().cloned());
            }
        }
    }
    folders
}

impl Desktop {
    pub(super) fn sync_folder_watch(&mut self) {
        let Some(watch) = &self.folder_watch.watch else {
            return;
        };
        let folders = displayed(&self.windows);
        if folders != self.folder_watch.watched {
            watch.watch(folders.iter().cloned());
            self.folder_watch.watched = folders;
        }
    }

    pub(super) fn folders_changed(&mut self) -> Task<Message> {
        let Some(watch) = &self.folder_watch.watch else {
            return Task::none();
        };
        let changed = watch.take_changes().folders;
        self.apply_folder_changes(&changed)
    }

    pub(super) fn apply_folder_changes(&mut self, changed: &BTreeSet<PathBuf>) -> Task<Message> {
        if changed.is_empty() {
            return Task::none();
        }
        let mut rescans = Vec::new();
        let mut columns = Vec::new();
        for (id, window) in &mut self.windows {
            for (index, tab) in window.tabs.iter_mut().enumerate() {
                let active = index == window.active;
                let current = tab
                    .browser
                    .as_ref()
                    .is_some_and(|browser| changed.contains(browser.path()))
                    || (tab.loading && changed.contains(&tab.target));
                if current {
                    if tab.loading || tab.watch_scanning || !active || tab.browser.is_none() {
                        // A loading scan may already have read the folder; an
                        // inactive tab refreshes when it is shown again.
                        tab.watch_stale = true;
                    } else {
                        rescans.push((*id, tab.id));
                    }
                }
                if active && !tab.loading {
                    columns.extend(
                        tab.columns
                            .keys()
                            .filter(|path| changed.contains(*path))
                            .map(|path| (*id, tab.id, path.clone())),
                    );
                }
            }
        }
        let mut tasks: Vec<_> = rescans
            .into_iter()
            .map(|(window, tab)| self.watch_rescan(window, tab))
            .collect();
        tasks.extend(
            columns
                .into_iter()
                .map(|(window, tab, path)| self.rescan_column(window, tab, path)),
        );
        Task::batch(tasks)
    }

    /// Re-read the tab's current folder without starting a navigation.
    pub(super) fn watch_rescan(&mut self, window: window::Id, tab_id: u64) -> Task<Message> {
        let Some(tab) = self
            .windows
            .get_mut(&window)
            .and_then(|window| window.tabs.iter_mut().find(|tab| tab.id == tab_id))
        else {
            return Task::none();
        };
        if tab.loading || tab.browser.is_none() {
            tab.watch_stale = true;
            return Task::none();
        }
        tab.watch_stale = false;
        tab.watch_scanning = true;
        tab.watch_request += 1;
        tab.watch_cancel.cancel();
        tab.watch_cancel = Cancellation::default();
        let (revision, request, cancellation) =
            (tab.revision, tab.watch_request, tab.watch_cancel.clone());
        let path = tab.path().to_owned();
        Task::perform(
            async move {
                Directory::read_cancellable(path, &cancellation).map_err(|error| {
                    (
                        error.kind() == io::ErrorKind::Interrupted,
                        error.to_string(),
                    )
                })
            },
            move |result| Message::WatchLoaded {
                window,
                tab: tab_id,
                revision,
                request,
                result: Arc::new(result),
            },
        )
    }

    pub(super) fn finish_watch_rescan(
        &mut self,
        window: window::Id,
        tab_id: u64,
        revision: u64,
        request: u64,
        result: Arc<Result<Directory, (bool, String)>>,
    ) -> Task<Message> {
        let Some(tab) = self
            .windows
            .get_mut(&window)
            .and_then(|window| window.tabs.iter_mut().find(|tab| tab.id == tab_id))
            .filter(|tab| tab.revision == revision && tab.watch_request == request)
        else {
            return Task::none();
        };
        tab.watch_scanning = false;
        match Arc::try_unwrap(result).unwrap_or_else(|shared| (*shared).clone()) {
            Ok(directory) => {
                if let Some(browser) = tab.browser.as_mut()
                    && browser.refresh(directory)
                    && tab.watch_error
                {
                    tab.watch_error = false;
                    tab.error = None;
                }
            }
            Err((true, _)) => {}
            Err((false, error)) => {
                tab.error = Some(error);
                tab.watch_error = true;
            }
        }
        if tab.watch_stale
            && self
                .active(window)
                .is_some_and(|active| active.id == tab_id)
        {
            return self.watch_rescan(window, tab_id);
        }
        Task::none()
    }

    /// Replace an ancestor column's snapshot in place, keeping its scroll.
    fn rescan_column(&mut self, window: window::Id, tab_id: u64, path: PathBuf) -> Task<Message> {
        let Some(tab) = self
            .windows
            .get_mut(&window)
            .and_then(|window| window.tabs.iter_mut().find(|tab| tab.id == tab_id))
        else {
            return Task::none();
        };
        tab.next_column_request += 1;
        let request = tab.next_column_request;
        let revision = tab.revision;
        let Some(column) = tab.columns.get_mut(&path) else {
            return Task::none();
        };
        column.cancellation.cancel();
        column.cancellation = Cancellation::default();
        column.request = request;
        let cancellation = column.cancellation.clone();
        let result_path = path.clone();
        Task::perform(
            async move {
                Directory::read_cancellable(path, &cancellation).map_err(|error| error.to_string())
            },
            move |result| Message::ColumnLoaded {
                window,
                tab: tab_id,
                revision,
                path: result_path.clone(),
                request,
                result: Arc::new(result),
            },
        )
    }
}
