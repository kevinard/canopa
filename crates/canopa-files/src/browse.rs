//! Directory snapshots and width-independent browsing state.
use crate::preferences::{SortField, ViewMode, ViewSettings};
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
    /// What this person may do with the item (its target, for a link), from
    /// its mode bits. Entries saved before this was recorded allow both.
    #[serde(default)]
    pub access: Access,
}

/// Whether the current user may read and change an item, as its permission
/// bits say. Access-control lists are not consulted: it marks emblems only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Access {
    pub readable: bool,
    pub writable: bool,
}

impl Default for Access {
    fn default() -> Self {
        Self {
            readable: true,
            writable: true,
        }
    }
}

impl Access {
    fn of(metadata: &fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        static IDENTITY: std::sync::OnceLock<(u32, Vec<u32>)> = std::sync::OnceLock::new();
        let (user, groups) = IDENTITY.get_or_init(|| {
            let mut groups: Vec<u32> = rustix::process::getgroups()
                .unwrap_or_default()
                .into_iter()
                .map(|group| group.as_raw())
                .collect();
            groups.push(rustix::process::getegid().as_raw());
            (rustix::process::geteuid().as_raw(), groups)
        });
        if *user == 0 {
            return Self::default();
        }
        let mode = metadata.mode();
        let bits = if metadata.uid() == *user {
            mode >> 6
        } else if groups.contains(&metadata.gid()) {
            mode >> 3
        } else {
            mode
        };
        Self {
            readable: bits & 0o4 != 0,
            writable: bits & 0o2 != 0,
        }
    }
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
        // A link's own mode means nothing; its target's says what it allows.
        let target = metadata
            .is_symlink()
            .then(|| fs::metadata(&path).ok())
            .flatten();
        let directory = metadata.is_dir() || target.as_ref().is_some_and(fs::Metadata::is_dir);
        let access = if metadata.is_symlink() {
            target.as_ref().map(Access::of).unwrap_or_default()
        } else {
            Access::of(&metadata)
        };
        Ok(Self {
            path,
            kind,
            size: metadata.len(),
            modified: metadata.modified().ok(),
            directory,
            access,
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
    /// Order the entries as `view` sorts them, as when this folder is shown.
    pub fn sort(&mut self, view: &ViewSettings) {
        sort_entries(&mut self.entries, view);
    }

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

/// A name ordered the way people count: "file2" before "file10". Runs of
/// ASCII digits compare by value, other text case-insensitively; at equal
/// value, fewer leading zeros come first ("7" before "07").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NaturalName(Vec<NameSegment>);

#[derive(Clone, Debug, PartialEq, Eq)]
enum NameSegment {
    /// Digits without leading zeros, and how many zeros there were.
    Number {
        digits: String,
        zeros: usize,
    },
    Text(String),
}

impl NaturalName {
    pub fn new(name: &str) -> Self {
        let mut segments = Vec::new();
        let mut chars = name.char_indices().peekable();
        while let Some(&(start, ch)) = chars.peek() {
            let numeric = ch.is_ascii_digit();
            let mut end = start;
            while let Some(&(index, ch)) = chars.peek() {
                if ch.is_ascii_digit() != numeric {
                    break;
                }
                end = index + ch.len_utf8();
                chars.next();
            }
            let run = &name[start..end];
            segments.push(if numeric {
                let digits = run.trim_start_matches('0');
                NameSegment::Number {
                    zeros: run.len() - digits.len(),
                    digits: digits.to_owned(),
                }
            } else {
                NameSegment::Text(run.to_lowercase())
            });
        }
        Self(segments)
    }
}

impl Ord for NameSegment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use NameSegment::{Number, Text};
        match (self, other) {
            (
                Number { digits, zeros },
                Number {
                    digits: other_digits,
                    zeros: other_zeros,
                },
            ) => digits
                .len()
                .cmp(&other_digits.len())
                .then_with(|| digits.cmp(other_digits))
                .then_with(|| zeros.cmp(other_zeros)),
            // Numbers lead text, as digits lead letters in plain ordering.
            (Number { .. }, Text(_)) => std::cmp::Ordering::Less,
            (Text(_), Number { .. }) => std::cmp::Ordering::Greater,
            (Text(text), Text(other)) => text.cmp(other),
        }
    }
}

impl PartialOrd for NameSegment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NaturalName {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for NaturalName {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum SortKey {
    Name(NaturalName),
    Size(u64),
    Modified(Option<SystemTime>),
    Kind(String),
}

fn sort_entries(entries: &mut [Entry], view: &ViewSettings) {
    let key = |entry: &Entry| {
        let primary = match view.sort {
            SortField::Name => SortKey::Name(NaturalName::new(&entry.name())),
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
        (primary, NaturalName::new(&entry.name()), entry.path.clone())
    };
    // Folders lead unless the view mixes them with files in one order.
    let group = |entry: &Entry| view.folders_first && !entry.is_directory();
    if view.descending {
        entries.sort_by_cached_key(|entry| (group(entry), std::cmp::Reverse(key(entry))));
    } else {
        entries.sort_by_cached_key(|entry| (group(entry), key(entry)));
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
    /// Listings of the folders opened in place in List view's tree, by path.
    /// Only those reachable through open folders from this folder are shown.
    tree: BTreeMap<PathBuf, Directory>,
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
            tree: BTreeMap::new(),
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
        for directory in self.tree.values_mut() {
            sort_entries(&mut directory.entries, &view);
        }
        let shown = self.tree_view();
        self.view = view;
        if shown != self.tree_view() {
            self.prune_selection();
        }
        Ok(())
    }

    /// Whether folders open in place, as a tree: List view asked for it, and
    /// no search replaces the folder's listing.
    pub fn tree_view(&self) -> bool {
        self.view.mode == ViewMode::List && self.view.tree && self.search.is_none()
    }

    /// Whether the folder at `path` is open in the tree.
    pub fn is_expanded(&self, path: &Path) -> bool {
        self.tree_view() && self.tree.contains_key(path)
    }

    /// How deep `path` sits below this folder in the tree; 0 for its own items.
    pub fn depth(&self, path: &Path) -> usize {
        if !self.tree_view() {
            return 0;
        }
        path.strip_prefix(self.path()).map_or(0, |relative| {
            relative.components().count().saturating_sub(1)
        })
    }

    /// Open a listed folder in place with its prepared listing. A folder that
    /// is not listed, or a listing for elsewhere, changes nothing.
    pub fn expand(&mut self, mut directory: Directory) -> bool {
        if !self.tree_view()
            || !self
                .visible_entries()
                .iter()
                .any(|entry| entry.path == directory.path && entry.is_directory())
        {
            return false;
        }
        sort_entries(&mut directory.entries, &self.view);
        self.tree.insert(directory.path.clone(), directory);
        true
    }

    /// Replace an open folder's listing after its contents changed.
    pub fn refresh_expanded(&mut self, mut directory: Directory) -> bool {
        let Some(old) = self.tree.get_mut(&directory.path) else {
            return false;
        };
        sort_entries(&mut directory.entries, &self.view);
        *old = directory;
        self.prune_selection();
        true
    }

    /// Close an open folder and every folder open inside it. Selection inside
    /// moves to the closed folder, so the keyboard keeps its place.
    pub fn collapse(&mut self, path: &Path) {
        self.tree.retain(|open, _| !open.starts_with(path));
        let inside = |candidate: &Path| candidate != path && candidate.starts_with(path);
        let folder = self.active_state_mut();
        let moved = folder.focus.as_deref().is_some_and(inside)
            || folder.selected.iter().any(|selected| inside(selected));
        folder.selected.retain(|selected| !inside(selected));
        if moved {
            folder.selected.insert(path.to_owned());
            folder.focus = Some(path.to_owned());
            folder.anchor = Some(path.to_owned());
        }
        self.prune_selection();
    }

    /// The listings of the folders open in the tree.
    pub fn expanded_listings(&self) -> impl Iterator<Item = &Directory> {
        self.tree.values()
    }

    /// The open folders, outermost first, to read again on a refresh.
    pub fn expanded(&self) -> Vec<PathBuf> {
        self.tree.keys().cloned().collect()
    }

    /// Whether `path` is one of the rows this folder lists, at any depth.
    pub fn lists(&self, path: &Path) -> bool {
        path.parent() == Some(self.path())
            || self.search.is_some()
            || self.tree_view()
                && path.starts_with(self.path())
                && self
                    .visible_entries()
                    .iter()
                    .any(|entry| entry.path == path)
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
            let listed = self.directory.visible_with(self.show_hidden, |path| {
                self.folder.navigation_child.as_deref() == Some(path)
                    || self.folder.revealed.contains(path)
            });
            if !self.tree_view() || self.tree.is_empty() {
                return listed;
            }
            let mut rows = Vec::with_capacity(listed.len());
            self.flatten(listed, &mut rows);
            rows
        }
    }

    /// `entries` with each open folder's own rows after it, depth first.
    fn flatten<'a>(&'a self, entries: Vec<&'a Entry>, rows: &mut Vec<&'a Entry>) {
        for entry in entries {
            rows.push(entry);
            if let Some(directory) = self.tree.get(&entry.path).filter(|_| entry.is_directory()) {
                let revealed = |path: &Path| self.folder.revealed.contains(path);
                self.flatten(directory.visible_with(self.show_hidden, revealed), rows);
            }
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
        // Another folder starts with its folders closed.
        self.tree.clear();
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
