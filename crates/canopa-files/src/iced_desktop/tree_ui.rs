//! List view's tree: folders open in place under their row. Their listings are
//! read on workers, like Columns view's ancestors, and installed only while the
//! tab still shows the folder they were asked for.
use super::*;

impl Desktop {
    /// Open or close the folder at `path` in the tree of tab `tab`.
    pub(super) fn toggle_tree(
        &mut self,
        id: window::Id,
        tab: u64,
        revision: u64,
        path: PathBuf,
    ) -> Task<Message> {
        let Some(current) = self.active_mut(id).filter(|current| {
            current.id == tab && current.revision == revision && !current.loading
        }) else {
            return Task::none();
        };
        let Some(browser) = current.browser.as_mut() else {
            return Task::none();
        };
        if browser.is_expanded(&path) {
            browser.collapse(&path);
            return Task::none();
        }
        if !browser.tree_view() || !browser.lists(&path) {
            return Task::none();
        }
        read_tree(id, tab, revision, vec![path], false)
    }

    /// Read the open folders of window `id`'s tree again, after its folder was
    /// refreshed: their contents may have changed with it.
    pub(super) fn reload_tree(&mut self, id: window::Id) -> Task<Message> {
        let Some(tab) = self.active(id).filter(|tab| !tab.loading) else {
            return Task::none();
        };
        let Some(browser) = tab.browser.as_ref().filter(|browser| browser.tree_view()) else {
            return Task::none();
        };
        let open = browser.expanded();
        if open.is_empty() {
            return Task::none();
        }
        read_tree(id, tab.id, tab.revision, open, true)
    }

    pub(super) fn tree_loaded(
        &mut self,
        id: window::Id,
        tab: u64,
        revision: u64,
        refresh: bool,
        result: Arc<Result<Directory, String>>,
    ) -> Task<Message> {
        let Some(current) = self.windows.get_mut(&id).and_then(|window| {
            window
                .tabs
                .iter_mut()
                .find(|current| current.id == tab && current.revision == revision)
        }) else {
            return Task::none();
        };
        let Some(browser) = current.browser.as_mut() else {
            return Task::none();
        };
        match Arc::try_unwrap(result).unwrap_or_else(|shared| (*shared).clone()) {
            Ok(directory) if refresh => {
                browser.refresh_expanded(directory);
            }
            Ok(directory) => {
                browser.expand(directory);
            }
            // A folder that cannot be read again closes; one that could not
            // be opened says why.
            Err(error) => {
                if !refresh {
                    current.error = Some(error);
                }
            }
        }
        Task::none()
    }

    /// Right opens the focused folder in place and Left closes it, or goes to
    /// the folder holding a nested row. `None` leaves the key to its command.
    pub(super) fn tree_key(&mut self, id: window::Id, name: &str) -> Option<Task<Message>> {
        let tab = self.active_mut(id).filter(|tab| tab.location.is_none())?;
        let (tab_id, revision) = (tab.id, tab.revision);
        let browser = tab.browser.as_mut().filter(|browser| browser.tree_view())?;
        let focused = browser.focused()?.to_owned();
        let folder = browser
            .visible_entries()
            .iter()
            .find(|entry| entry.path == focused)
            .is_some_and(|entry| entry.is_directory());
        match name {
            "Right" if folder && !browser.is_expanded(&focused) => {
                Some(self.toggle_tree(id, tab_id, revision, focused))
            }
            "Left" if browser.is_expanded(&focused) => {
                browser.collapse(&focused);
                Some(Task::none())
            }
            "Left" if browser.depth(&focused) > 0 => {
                let parent = focused.parent()?.to_owned();
                browser.select(&parent, SelectionMode::Replace);
                Some(self.reveal_focus(id))
            }
            _ => None,
        }
    }
}

fn read_tree(
    id: window::Id,
    tab: u64,
    revision: u64,
    paths: Vec<PathBuf>,
    refresh: bool,
) -> Task<Message> {
    Task::batch(paths.into_iter().map(|path| {
        Task::perform(
            async move { Directory::read(path).map_err(|error| error.to_string()) },
            move |result| Message::TreeLoaded {
                window: id,
                tab,
                revision,
                refresh,
                result: Arc::new(result),
            },
        )
    }))
}
