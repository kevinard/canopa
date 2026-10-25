//! Directory snapshots and width-independent browsing state.
use crate::preferences::{SortField, ViewSettings};
use crate::search::SearchResults;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    #[serde(with = "crate::path_serde")]
    pub path: PathBuf,
    pub kind: EntryKind,
    pub size: u64,
    #[serde(with = "crate::time_serde")]
    pub modified: Option<SystemTime>,
    directory: bool,
}

impl Entry {
    pub fn read(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = std::path::absolute(path)?;
        let metadata = fs::symlink_metadata(&path)?;
        let kind = if metadata.is_symlink() {
            EntryKind::Symlink
        } else if metadata.is_dir() {
            EntryKind::Directory
        } else if metadata.is_file() {
            EntryKind::File
        } else {
            EntryKind::Other
        };
        let directory = metadata.is_dir()
            || (metadata.is_symlink() && fs::metadata(&path).is_ok_and(|m| m.is_dir()));
        Ok(Self {
            path,
            kind,
            size: metadata.len(),
            modified: metadata.modified().ok(),
            directory,
        })
    }

    pub fn is_directory(&self) -> bool {
        self.directory
    }

    pub fn name(&self) -> std::borrow::Cow<'_, str> {
        self.path
            .file_name()
            .unwrap_or(self.path.as_os_str())
            .to_string_lossy()
    }

    pub fn is_hidden(&self) -> bool {
        self.path
            .file_name()
            .is_some_and(|name| name.as_encoded_bytes().first() == Some(&b'.'))
    }
}

/// Prepared on a worker; installing it never performs filesystem I/O.
#[derive(Clone, Debug)]
pub struct Directory {
    pub path: PathBuf,
    /// Actual location observed by this snapshot, independent of display aliases.
    pub resolved_path: PathBuf,
    pub entries: Vec<Entry>,
    pub failures: Vec<ReadFailure>,
}

#[derive(Clone, Debug)]
pub struct ReadFailure {
    pub path: PathBuf,
    pub message: String,
}

impl Directory {
    pub fn read(path: impl AsRef<Path>) -> io::Result<Self> {
        Self::read_cancellable(path, &crate::Cancellation::default())
    }

    /// Stop abandoned scans between entries without publishing a partial folder.
    pub fn read_cancellable(
        path: impl AsRef<Path>,
        cancellation: &crate::Cancellation,
    ) -> io::Result<Self> {
        let cancelled = || io::Error::new(io::ErrorKind::Interrupted, "folder scan cancelled");
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        let path = std::path::absolute(path)?;
        let resolved_path = fs::canonicalize(&path)?;
        let mut entries = Vec::new();
        let mut failures = Vec::new();
        for item in fs::read_dir(&path)? {
            if cancellation.is_cancelled() {
                return Err(cancelled());
            }
            let item = item.map(|item| item.path());
            match item {
                Ok(item) => match Entry::read(&item) {
                    Ok(entry) => entries.push(entry),
                    Err(error) => failures.push(ReadFailure {
                        path: item,
                        message: error.to_string(),
                    }),
                },
                Err(error) => failures.push(ReadFailure {
                    path: path.clone(),
                    message: error.to_string(),
                }),
            }
        }
        if cancellation.is_cancelled() {
            return Err(cancelled());
        }
        sort_entries(&mut entries, &ViewSettings::default());
        Ok(Self {
            path,
            resolved_path,
            entries,
            failures,
        })
    }

    /// Keep an explicitly browsed child in its hierarchy without revealing its
    /// hidden siblings. The current folder uses the same rule after going up.
    pub fn visible_entries(
        &self,
        show_hidden: bool,
        navigation_child: Option<&Path>,
    ) -> Vec<&Entry> {
        self.visible_with(show_hidden, |path| navigation_child == Some(path))
    }

    fn visible_with(&self, show_hidden: bool, explicit: impl Fn(&Path) -> bool) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| show_hidden || !entry.is_hidden() || explicit(&entry.path))
            .collect()
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum SortKey {
    Name(String),
    Size(u64),
    Modified(Option<SystemTime>),
    Kind(String),
}

fn sort_entries(entries: &mut [Entry], view: &ViewSettings) {
    let key = |entry: &Entry| {
        let primary = match view.sort {
            SortField::Name => SortKey::Name(entry.name().to_lowercase()),
            SortField::Size => SortKey::Size(entry.size),
            SortField::Modified => SortKey::Modified(entry.modified),
            SortField::Kind => SortKey::Kind(
                entry
                    .path
                    .extension()
                    .map(|ext| ext.to_string_lossy().to_lowercase())
                    .unwrap_or_default(),
            ),
        };
        (primary, entry.name().to_lowercase(), entry.path.clone())
    };
    if view.descending {
        entries.sort_by_cached_key(|entry| (!entry.is_directory(), std::cmp::Reverse(key(entry))));
    } else {
        entries.sort_by_cached_key(|entry| (!entry.is_directory(), key(entry)));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionMode {
    Replace,
    Toggle,
    Extend,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryDirection {
    Back,
    Forward,
}

#[derive(Clone, Debug, Default)]
struct FolderState {
    navigation_child: Option<PathBuf>,
    revealed: BTreeSet<PathBuf>,
    selected: BTreeSet<PathBuf>,
    focus: Option<PathBuf>,
    anchor: Option<PathBuf>,
    scroll: f32,
}

#[derive(Clone, Debug)]
struct SearchView {
    results: SearchResults,
    folder: FolderState,
}

/// One tab's browsing state. Visible column count is a projection, not navigation.
#[derive(Clone, Debug)]
pub struct Browser {
    directory: Directory,
    folder: FolderState,
    remembered: BTreeMap<PathBuf, FolderState>,
    show_hidden: bool,
    search: Option<SearchView>,
    history: Vec<PathBuf>,
    history_index: usize,
    view: ViewSettings,
}

impl Browser {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self::new(Directory::read(path)?))
    }

    pub fn new(directory: Directory) -> Self {
        let history = vec![directory.path.clone()];
        Self {
            directory,
            folder: FolderState::default(),
            remembered: BTreeMap::new(),
            show_hidden: false,
            search: None,
            history,
            history_index: 0,
            view: ViewSettings::default(),
        }
    }

    pub fn directory(&self) -> &Directory {
        &self.directory
    }

    pub fn view(&self) -> &ViewSettings {
        &self.view
    }

    pub fn set_view(&mut self, view: ViewSettings) -> io::Result<()> {
        view.validate()?;
        sort_entries(&mut self.directory.entries, &view);
        if let Some(search) = self.search.as_mut() {
            sort_entries(&mut search.results.entries, &view);
        }
        self.view = view;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.directory.path
    }

    pub fn parent(&self) -> Option<&Path> {
        self.path().parent()
    }

    pub fn focused(&self) -> Option<&Path> {
        self.active_state().focus.as_deref()
    }

    pub fn selected(&self) -> &BTreeSet<PathBuf> {
        &self.active_state().selected
    }

    /// Reconcile native pointer selection without resetting the keyboard range anchor.
    pub fn set_selection(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        self.active_state_mut().selected = paths.into_iter().collect();
        self.prune_selection();
        let folder = self.active_state_mut();
        if folder.focus.is_none() {
            folder.focus = folder.selected.first().cloned();
            folder.anchor.clone_from(&folder.focus);
        }
    }

    pub fn focus_entry(&mut self, path: &Path, reset_anchor: bool) {
        if !self
            .visible_entries()
            .iter()
            .any(|entry| entry.path == path)
        {
            return;
        }
        let folder = self.active_state_mut();
        folder.focus = Some(path.to_owned());
        if reset_anchor || folder.anchor.is_none() {
            folder.anchor = Some(path.to_owned());
        }
    }

    /// Reveal an explicit external selection, including only its hidden items.
    /// It changes this folder session, never the global hidden-file preference.
    pub fn reveal_selection(&mut self, paths: Vec<PathBuf>) {
        self.close_search();
        self.folder.revealed = paths.iter().cloned().collect();
        self.set_selection(paths.clone());
        if let Some(path) = paths.iter().find(|p| self.folder.selected.contains(*p)) {
            self.folder.focus = Some(path.clone());
            self.folder.anchor = Some(path.clone());
        }
    }

    pub fn visible_entries(&self) -> Vec<&Entry> {
        if let Some(search) = &self.search {
            search
                .results
                .entries
                .iter()
                .filter(|entry| self.show_hidden || !entry.is_hidden())
                .collect()
        } else {
            self.directory.visible_with(self.show_hidden, |path| {
                self.folder.navigation_child.as_deref() == Some(path)
                    || self.folder.revealed.contains(path)
            })
        }
    }

    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    pub fn set_show_hidden(&mut self, show: bool) {
        self.show_hidden = show;
        if !show {
            self.folder.navigation_child = None;
            self.folder.revealed.clear();
            for folder in self.remembered.values_mut() {
                folder.navigation_child = None;
                folder.revealed.clear();
            }
        }
        self.prune_selection();
    }

    pub fn select_all(&mut self) {
        let selected = self
            .visible_entries()
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
        let first = self
            .visible_entries()
            .first()
            .map(|entry| entry.path.clone());
        let folder = self.active_state_mut();
        folder.selected = selected;
        if folder.focus.is_none() {
            folder.focus = first;
            folder.anchor.clone_from(&folder.focus);
        }
    }

    pub fn clear_selection(&mut self) {
        let folder = self.active_state_mut();
        folder.selected.clear();
        folder.focus = None;
        folder.anchor = None;
    }

    pub fn scroll(&self) -> f32 {
        self.active_state().scroll
    }

    pub fn set_scroll(&mut self, offset: f32) {
        if offset.is_finite() {
            self.active_state_mut().scroll = offset.max(0.0);
        }
    }

    pub fn select(&mut self, path: &Path, mode: SelectionMode) {
        if !self
            .visible_entries()
            .iter()
            .any(|entry| entry.path == path)
        {
            return;
        }
        let visible: Vec<_> = self
            .visible_entries()
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
        let folder = self.active_state_mut();
        match mode {
            SelectionMode::Replace => {
                folder.selected.clear();
                folder.selected.insert(path.to_owned());
                folder.anchor = Some(path.to_owned());
            }
            SelectionMode::Toggle => {
                if !folder.selected.remove(path) {
                    folder.selected.insert(path.to_owned());
                }
                folder.anchor = Some(path.to_owned());
            }
            SelectionMode::Extend => {
                let target = visible.iter().position(|entry| entry == path).unwrap();
                let anchor = folder
                    .anchor
                    .as_ref()
                    .and_then(|anchor| visible.iter().position(|entry| entry == anchor))
                    .unwrap_or(target);
                folder.selected = visible[anchor.min(target)..=anchor.max(target)]
                    .iter()
                    .cloned()
                    .collect();
                if folder.anchor.is_none() {
                    folder.anchor = Some(path.to_owned());
                }
            }
        }
        folder.focus = Some(path.to_owned());
    }

    pub fn step(&mut self, delta: isize, mode: SelectionMode) {
        let visible = self.visible_entries();
        if visible.is_empty() {
            return;
        }
        let position = self
            .focused()
            .and_then(|focus| visible.iter().position(|entry| entry.path == focus));
        let position = position
            .map_or(0, |position| position.saturating_add_signed(delta))
            .min(visible.len() - 1);
        let path = visible[position].path.clone();
        self.select(&path, mode);
    }

    pub fn select_prefix(&mut self, prefix: &str) -> bool {
        if prefix.is_empty() {
            return false;
        }
        let prefix = prefix.to_lowercase();
        let path = self
            .visible_entries()
            .iter()
            .find(|entry| entry.name().to_lowercase().starts_with(&prefix))
            .map(|entry| entry.path.clone());
        if let Some(path) = path {
            self.select(&path, SelectionMode::Replace);
            return true;
        }
        false
    }

    pub fn visit(&mut self, directory: Directory) {
        if directory.path != self.path() {
            self.history.truncate(self.history_index + 1);
            self.history.push(directory.path.clone());
            if self.history.len() > 256 {
                self.history.remove(0);
            }
            self.history_index = self.history.len() - 1;
        }
        self.install(directory);
    }

    pub fn history_target(&self, direction: HistoryDirection) -> Option<&Path> {
        let index = match direction {
            HistoryDirection::Back => self.history_index.checked_sub(1)?,
            HistoryDirection::Forward => self.history_index.checked_add(1)?,
        };
        self.history.get(index).map(PathBuf::as_path)
    }

    pub fn visit_history(&mut self, directory: Directory, direction: HistoryDirection) -> bool {
        if self.history_target(direction) != Some(directory.path.as_path()) {
            return false;
        }
        self.history_index = match direction {
            HistoryDirection::Back => self.history_index - 1,
            HistoryDirection::Forward => self.history_index + 1,
        };
        self.install(directory);
        true
    }

    /// An out-of-date scan for another folder must never replace this tab's view.
    pub fn refresh(&mut self, mut directory: Directory) -> bool {
        if directory.path != self.path() {
            return false;
        }
        sort_entries(&mut directory.entries, &self.view);
        self.directory = directory;
        self.prune_selection();
        true
    }

    fn install(&mut self, mut directory: Directory) {
        self.close_search();
        let previous = self.directory.path.clone();
        self.remembered
            .insert(previous.clone(), std::mem::take(&mut self.folder));
        self.folder = self
            .remembered
            .get(&directory.path)
            .cloned()
            .unwrap_or_default();
        sort_entries(&mut directory.entries, &self.view);
        self.directory = directory;
        let returning_to_parent = previous.parent() == Some(self.path());
        if returning_to_parent {
            self.folder.navigation_child = Some(previous.clone());
        }
        self.prune_selection();
        if returning_to_parent {
            self.select(&previous, SelectionMode::Replace);
        }
    }

    fn prune_selection(&mut self) {
        let visible: BTreeSet<_> = self
            .visible_entries()
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
        let folder = self.active_state_mut();
        folder.selected.retain(|path| visible.contains(path));
        if folder
            .focus
            .as_ref()
            .is_some_and(|path| !visible.contains(path))
        {
            folder.focus = None;
        }
        if folder
            .anchor
            .as_ref()
            .is_some_and(|path| !visible.contains(path))
        {
            folder.anchor.clone_from(&folder.focus);
        }
    }

    fn active_state(&self) -> &FolderState {
        self.search
            .as_ref()
            .map_or(&self.folder, |search| &search.folder)
    }

    fn active_state_mut(&mut self) -> &mut FolderState {
        self.search
            .as_mut()
            .map_or(&mut self.folder, |search| &mut search.folder)
    }

    pub fn search(&self) -> Option<&SearchResults> {
        self.search.as_ref().map(|search| &search.results)
    }

    /// Reject results belonging to another location. The UI also guards request revisions.
    pub fn show_search(&mut self, mut results: SearchResults) -> bool {
        if results.root != self.path() {
            return false;
        }
        sort_entries(&mut results.entries, &self.view);
        let folder = self
            .search
            .take()
            .filter(|search| {
                search.results.query == results.query && search.results.scope == results.scope
            })
            .map(|search| search.folder)
            .unwrap_or_default();
        self.search = Some(SearchView { results, folder });
        self.prune_selection();
        true
    }

    pub fn close_search(&mut self) {
        self.search = None;
        self.prune_selection();
    }

    pub fn column_paths(&self, width: f32) -> Vec<PathBuf> {
        let available = if width.is_finite() { width } else { 0.0 };
        let available = available - if available >= 960.0 { 160.0 } else { 0.0 };
        self.column_paths_for_content(available)
    }

    /// Project columns into the actual content area after configured chrome.
    pub fn column_paths_for_content(&self, width: f32) -> Vec<PathBuf> {
        let available = if width.is_finite() { width } else { 0.0 };
        let count = ((available / f32::from(self.view.column_width)).floor() as usize).clamp(1, 8);
        let mut paths: Vec<_> = self
            .path()
            .ancestors()
            .take(count)
            .map(Path::to_path_buf)
            .collect();
        paths.reverse();
        paths
    }
}
