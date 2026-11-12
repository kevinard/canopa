//! Visible ancestor snapshots have independent, cancellable worker lifetimes.
use super::*;

pub(super) struct ColumnSnapshot {
    pub(super) request: u64,
    pub(super) cancellation: Cancellation,
    pub result: Option<Result<Directory, String>>,
    pub scroll: f32,
}

impl Drop for ColumnSnapshot {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

impl Desktop {
    pub(super) fn load_columns(&mut self, id: window::Id) -> Task<Message> {
        let sidebar = self.configuration.settings.sidebar;
        let Some(window) = self.windows.get_mut(&id) else {
            return Task::none();
        };
        if !window.size_confirmed {
            return Task::none();
        }
        let reserved = window.panel_width();
        let Some(tab) = window.tabs.get_mut(window.active) else {
            return Task::none();
        };
        if tab.loading {
            return Task::none();
        }
        let paths = tab.column_paths(window.width, sidebar, reserved, &self.home);
        let wanted: Vec<_> = paths
            .into_iter()
            .filter(|path| path != tab.path())
            .collect();
        tab.columns.retain(|path, _| wanted.contains(path));
        let mut tasks = Vec::new();
        for path in wanted {
            if tab.columns.contains_key(&path) {
                continue;
            }
            tab.next_column_request += 1;
            let request = tab.next_column_request;
            let cancellation = Cancellation::default();
            tab.columns.insert(
                path.clone(),
                ColumnSnapshot {
                    request,
                    cancellation: cancellation.clone(),
                    result: None,
                    scroll: 0.0,
                },
            );
            let (tab_id, revision) = (tab.id, tab.revision);
            let result_path = path.clone();
            tasks.push(Task::perform(
                async move {
                    Directory::read_cancellable(path, &cancellation)
                        .map_err(|error| error.to_string())
                },
                move |result| Message::ColumnLoaded {
                    window: id,
                    tab: tab_id,
                    revision,
                    path: result_path.clone(),
                    request,
                    result: Arc::new(result),
                },
            ));
        }
        Task::batch(tasks)
    }

    pub(super) fn finish_column(
        &mut self,
        id: window::Id,
        tab_id: u64,
        revision: u64,
        path: PathBuf,
        request: u64,
        result: Arc<Result<Directory, String>>,
    ) {
        if let Some(tab) = self.windows.get_mut(&id).and_then(|window| {
            window
                .tabs
                .iter_mut()
                .find(|tab| tab.id == tab_id && tab.revision == revision)
        }) && let Some(column) = tab
            .columns
            .get_mut(&path)
            .filter(|column| column.request == request)
        {
            column.result =
                Some(Arc::try_unwrap(result).unwrap_or_else(|shared| (*shared).clone()));
        }
    }
}
