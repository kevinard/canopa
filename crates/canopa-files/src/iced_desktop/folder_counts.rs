//! Item counts for listed folders, read off the renderer thread after the
//! listing is shown. Until a folder's count arrives its Size keeps the dash.
//! Counts follow the folder's modification time, so adding or removing a child
//! makes the next listing count it again.
use super::*;
use std::{collections::HashMap, time::SystemTime};

/// Folders counted in one worker pass.
const BATCH: usize = 256;
/// Counting stops here; the label then reads as a lower bound.
pub(super) const LIMIT: usize = 10_000;
/// Remembered folders before the cache starts over.
const CAPACITY: usize = 65_536;

#[derive(Clone, Debug)]
struct Count {
    modified: Option<SystemTime>,
    hidden: bool,
    /// `None` when the folder could not be read.
    items: Option<usize>,
}

#[derive(Default)]
pub(super) struct FolderCounts {
    counts: HashMap<PathBuf, Count>,
    running: bool,
}

/// A finished pass: each folder, the listing facts it was counted for, and its count.
#[derive(Clone, Debug)]
pub(super) struct Counted(Vec<(PathBuf, Option<SystemTime>, bool, Option<usize>)>);

impl FolderCounts {
    /// The count for `entry` as listed now, if one is known.
    pub fn items(&self, entry: &crate::browse::Entry, hidden: bool) -> Option<usize> {
        self.counts
            .get(&entry.path)
            .filter(|count| count.modified == entry.modified && count.hidden == hidden)
            .and_then(|count| count.items)
    }

    fn known(&self, entry: &crate::browse::Entry, hidden: bool) -> bool {
        self.counts
            .get(&entry.path)
            .is_some_and(|count| count.modified == entry.modified && count.hidden == hidden)
    }

    pub fn completed(&mut self, counted: Counted) {
        self.running = false;
        if self.counts.len() + counted.0.len() > CAPACITY {
            self.counts.clear();
        }
        for (path, modified, hidden, items) in counted.0 {
            self.counts.insert(
                path,
                Count {
                    modified,
                    hidden,
                    items,
                },
            );
        }
    }
}

/// The folder's visible children, stopping at [`LIMIT`].
fn count(path: &Path, hidden: bool) -> Option<usize> {
    use std::os::unix::ffi::OsStrExt;
    let entries = std::fs::read_dir(path).ok()?;
    Some(
        entries
            .filter_map(Result::ok)
            .filter(|entry| hidden || !entry.file_name().as_bytes().starts_with(b"."))
            .take(LIMIT)
            .count(),
    )
}

impl Desktop {
    /// Count folders shown with a Size detail in local listings, one bounded
    /// pass at a time. Network listings keep the dash rather than reading every
    /// child folder over the connection.
    pub(super) fn sync_folder_counts(&mut self) -> Task<Message> {
        if self.folder_counts.running {
            return Task::none();
        }
        let mut pending = Vec::new();
        'windows: for window in self.windows.values() {
            let Some(tab) = window.tabs.get(window.active) else {
                continue;
            };
            let Some(browser) = &tab.browser else {
                continue;
            };
            let view = browser.view();
            if tab.remote
                || tab.loading
                || view.mode != crate::preferences::ViewMode::List
                || !view.details.contains(&crate::preferences::Detail::Size)
            {
                continue;
            }
            let hidden = browser.show_hidden();
            for entry in browser.visible_entries() {
                if entry.is_directory() && !self.folder_counts.known(entry, hidden) {
                    pending.push((entry.path.clone(), entry.modified, hidden));
                    if pending.len() == BATCH {
                        break 'windows;
                    }
                }
            }
        }
        if pending.is_empty() {
            return Task::none();
        }
        self.folder_counts.running = true;
        Task::perform(
            async move {
                Counted(
                    pending
                        .into_iter()
                        .map(|(path, modified, hidden)| {
                            let items = count(&path, hidden);
                            (path, modified, hidden, items)
                        })
                        .collect(),
                )
            },
            Message::FolderCounts,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_visible_children_and_follow_the_folder_modification_time() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("folder");
        std::fs::create_dir(&folder).unwrap();
        for name in ["a", "b", ".hidden"] {
            std::fs::write(folder.join(name), b"x").unwrap();
        }
        assert_eq!(count(&folder, false), Some(2));
        assert_eq!(count(&folder, true), Some(3));
        assert_eq!(count(&root.path().join("missing"), false), None);

        let entry = crate::browse::Entry::read(&folder).unwrap();
        let mut counts = FolderCounts::default();
        counts.completed(Counted(vec![(
            entry.path.clone(),
            entry.modified,
            false,
            Some(2),
        )]));
        assert_eq!(counts.items(&entry, false), Some(2));
        assert_eq!(counts.items(&entry, true), None);
        let mut changed = entry.clone();
        changed.modified = None;
        assert_eq!(counts.items(&changed, false), None);
    }
}
