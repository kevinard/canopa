//! Adapts bounded search providers to the existing selectable Panel Menu.
mod clipboard;
mod personalization;
use super::{
    Message,
    messages::PanelEvent,
    panel_menu_session::{PanelMenuEntry, PanelMenuEntryTarget},
};
use crate::launcher::{self, Action, Batch, Config, FileIndex, NamedAction, ResultItem, Scope};
use futures_util::SinkExt;
use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub session: u64,
    pub generation: u64,
    pub revision: u64,
    pub provider: usize,
    pub result: Arc<ResultItem>,
    pub thumbnail: Option<iced::widget::image::Handle>,
}

impl Target {
    pub fn identity(&self) -> (usize, &str) {
        (self.provider, &self.result.id)
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    ClipboardChanged {
        session: u64,
        snapshot: Arc<launcher::clipboard::Snapshot>,
    },
    ClipboardFinished {
        session: u64,
        generation: u64,
        result: Result<Option<clipboard::Draft>, String>,
    },

    Preferences {
        session: u64,
        value: Arc<canopa_launcher::Preferences>,
        error: Option<String>,
    },
    Edited {
        session: u64,
        result: Result<Arc<canopa_launcher::Preferences>, String>,
    },
    Context {
        session: u64,
        windows: Vec<launcher::windows::Window>,
        history: Arc<launcher::history::Snapshot>,
        error: Option<String>,
    },
    ApplicationsForFile {
        session: u64,
        mode: u64,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
    },
    SelectionFinished {
        session: u64,
        generation: u64,
        key: Option<String>,
        result: Result<(), String>,
    },
    Indexed {
        session: u64,
        index: Arc<FileIndex>,
        refreshed: Instant,
    },
    Results {
        session: u64,
        generation: u64,
        provider: usize,
        index_revision: u64,
        batch: Batch,
    },
    ActionFinished {
        session: u64,
        generation: u64,
        result: Result<(), String>,
    },
}

#[derive(Debug, Clone)]
struct OpenWith {
    path: std::path::PathBuf,
    previous_query: String,
    mode: u64,
    apps: Option<Vec<crate::desktop_apps::DesktopApplication>>,
}

#[derive(Debug, Clone)]
pub(super) struct Session {
    pub id: u64,
    pub generation: u64,
    revision: u64,
    config: Arc<Config>,
    base_config: Arc<Config>,
    preferences: Arc<canopa_launcher::Preferences>,
    preferences_error: Option<String>,
    configuration_error: Option<String>,
    actions_mode: Option<personalization::Actions>,
    clipboard: Arc<launcher::clipboard::Snapshot>,
    naming: Option<clipboard::Naming>,
    alias_mode: Option<personalization::Alias>,
    apps: Vec<crate::desktop_apps::DesktopApplication>,
    app_results: Vec<PanelMenuEntry>,
    query: String,
    windows: Vec<launcher::windows::Window>,
    history: Arc<launcher::history::Snapshot>,
    history_error: Option<String>,
    open_with: Option<OpenWith>,
    mode_serial: u64,
    index: Option<Arc<FileIndex>>,
    index_revision: u64,
    refreshed: Option<Instant>,
    batches: BTreeMap<usize, Batch>,
    pub action_running: bool,
    action_error: String,
}

impl Session {
    pub fn new(config: Config, apps: Vec<crate::desktop_apps::DesktopApplication>) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let app_results = super::panel_menu_session::installed_app_entries(&apps, "");
        let (effective, configuration_error) =
            launcher::bundled::effective(&config, &Default::default());
        Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            generation: 0,
            revision: 0,
            base_config: Arc::new(config.clone()),
            config: Arc::new(effective),
            preferences: Arc::new(Default::default()),
            preferences_error: None,
            configuration_error,
            actions_mode: None,
            clipboard: Arc::default(),
            naming: None,
            alias_mode: None,
            apps,
            app_results,
            query: String::new(),
            windows: vec![],
            history: Arc::new(Default::default()),
            history_error: None,
            open_with: None,
            mode_serial: 0,
            index: None,
            index_revision: 0,
            refreshed: None,
            batches: BTreeMap::new(),
            action_running: false,
            action_error: String::new(),
        }
    }

    pub fn search_changed(&mut self, query: &str) {
        self.query = query.into();
        self.generation = self.generation.wrapping_add(1);
        self.revision = self.revision.wrapping_add(1);
        self.batches.clear();
        self.action_error.clear();
        let (scope, text) = self.query_scope();
        self.app_results = if query.len() <= launcher::MAX_QUERY_BYTES
            && matches!(scope, Scope::All | Scope::Apps)
        {
            super::panel_menu_session::installed_app_entries(
                self.open_with
                    .as_ref()
                    .and_then(|choice| choice.apps.as_deref())
                    .unwrap_or(if self.open_with.is_some() {
                        &[]
                    } else {
                        &self.apps
                    }),
                text,
            )
        } else {
            vec![]
        };
    }

    pub fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::ClipboardChanged { session, snapshot } if session == self.id => {
                self.clipboard = snapshot;
            }
            Event::ClipboardFinished {
                session,
                generation,
                result,
            } if session == self.id => {
                self.action_running = false;
                if generation == self.generation {
                    match result {
                        Ok(Some(draft)) => self.start_naming(draft),
                        Ok(None) => {
                            if let Some(query) = self.leave_child() {
                                self.search_changed(&query);
                            }
                        }
                        Err(error) => self.action_error = error,
                    }
                }
            }
            Event::Preferences {
                session,
                value,
                error,
            } if session == self.id => {
                self.set_preferences(value, error);
            }
            Event::Edited { session, result } if session == self.id => {
                self.action_running = false;
                match result {
                    Ok(value) => {
                        if let Some(query) = self.leave_child() {
                            self.query = query;
                        }
                        self.set_preferences(value, None);
                    }
                    Err(error) => self.action_error = error,
                }
            }
            Event::Context {
                session,
                windows,
                history,
                error,
            } if session == self.id => {
                if self.actions_mode.as_ref().and_then(|m|m.result.actions.first()).is_some_and(|a|matches!(a.action,Action::FocusWindow{id,..} if !windows.iter().any(|w|w.id==id)))&&let Some(query)=self.leave_child(){self.search_changed(&query);}
                self.windows = windows;
                if self.config.history {
                    self.history = history;
                    self.history_error = error;
                }
            }
            Event::ApplicationsForFile {
                session,
                mode,
                apps,
            } if session == self.id
                && self
                    .open_with
                    .as_ref()
                    .is_some_and(|choice| choice.mode == mode) =>
            {
                self.open_with.as_mut().unwrap().apps = Some(apps);
                let query = self.query.clone();
                self.search_changed(&query);
            }
            Event::Indexed {
                session,
                index,
                refreshed,
            } if session == self.id => {
                self.index = Some(index);
                self.refreshed = Some(refreshed);
                self.index_revision = self.index_revision.wrapping_add(1);
            }
            Event::Results {
                session,
                generation,
                provider,
                index_revision,
                batch,
            } if session == self.id
                && generation == self.generation
                && (provider != 1 || index_revision == self.index_revision) =>
            {
                self.batches.insert(provider, batch);
            }
            Event::ActionFinished {
                session,
                generation,
                result,
            } if session == self.id => {
                self.action_running = false;
                if generation == self.generation {
                    self.action_error = result.err().unwrap_or_default();
                }
            }
            _ => return false,
        }
        self.revision = self.revision.wrapping_add(1);
        true
    }

    pub fn accepts(&self, target: &Target) -> bool {
        target.session == self.id
            && target.generation == self.generation
            && target.revision == self.revision
            && !self.action_running
    }

    fn query_scope(&self) -> (Scope, &str) {
        if self.open_with.is_some() {
            (Scope::Apps, self.query.trim())
        } else {
            launcher::scope(&self.query, &self.config)
        }
    }
    pub fn needs_context(
        &self,
        windows: &[launcher::windows::Window],
        history: &Arc<launcher::history::Snapshot>,
        error: &Option<String>,
    ) -> bool {
        self.windows != windows
            || self.config.history
                && (!Arc::ptr_eq(&self.history, history) || &self.history_error != error)
    }
    pub fn tracks_history(&self) -> bool {
        self.config.history
    }
    pub fn enter_open_with(&mut self, path: std::path::PathBuf) {
        self.mode_serial = self.mode_serial.wrapping_add(1);
        let previous_query = self
            .actions_mode
            .take()
            .map(|m| m.previous_query)
            .unwrap_or_else(|| self.query.clone());
        self.open_with = Some(OpenWith {
            path,
            previous_query,
            mode: self.mode_serial,
            apps: None,
        });
    }
    pub fn leave_open_with(&mut self) -> Option<String> {
        self.open_with.take().map(|choice| choice.previous_query)
    }
    pub fn prompt(&self) -> Option<String> {
        if self.naming.is_some() {
            return Some("Snippet name".into());
        }
        if let Some(mode) = &self.actions_mode {
            return Some(format!("Actions for {}", mode.result.label));
        }
        if let Some(mode) = &self.alias_mode {
            return Some(format!("Alias for {}", mode.shortcut.label));
        }
        self.open_with.as_ref().map(|choice| {
            format!(
                "Open with — {}",
                choice
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            )
        })
    }

    pub fn entries(&self) -> Vec<PanelMenuEntry> {
        if let Some(entries) = self.child_entries() {
            return entries;
        }
        if self.query.len() > launcher::MAX_QUERY_BYTES {
            return vec![];
        }
        let (scope, text) = self.query_scope();
        if matches!(scope, Scope::Clipboard | Scope::Snippets) {
            return self.clipboard_entries(scope, text);
        }
        let mut entries = Vec::new();
        if self.open_with.is_none()
            && let Some(batch) = self.batches.get(&0)
        {
            entries.extend(
                batch
                    .results
                    .iter()
                    .cloned()
                    .map(|result| self.entry(0, result)),
            );
        }
        if self.open_with.is_none() {
            for (provider, batch) in &self.batches {
                if *provider >= 2
                    && self
                        .config
                        .extensions
                        .get(*provider - 2)
                        .is_some_and(|extension| extension.id.starts_with("builtin:"))
                {
                    entries.extend(
                        batch
                            .results
                            .iter()
                            .cloned()
                            .map(|result| self.entry(*provider, result)),
                    );
                }
            }
        }
        let mut native = self.shortcut_entries();
        if matches!(scope, Scope::All | Scope::Windows) {
            native.extend(
                launcher::windows::results(&self.windows, &self.apps, text)
                    .into_iter()
                    .map(|result| self.entry(usize::MAX - 2, result)),
            );
        }
        if matches!(scope, Scope::All | Scope::Apps) {
            native.extend(self.app_results.iter().map(|entry| {
                let PanelMenuEntryTarget::InstalledApp(id) = &entry.target else {
                    unreachable!()
                };
                let (description, action) = if let Some(choice) = &self.open_with {
                    (
                        format!("Open with · {}", choice.path.display()),
                        Action::OpenWith {
                            path: choice.path.clone(),
                            application: id.clone(),
                        },
                    )
                } else {
                    ("Application".into(), Action::Application { id: id.clone() })
                };
                self.entry(
                    usize::MAX,
                    ResultItem {
                        id: id.clone(),
                        label: entry.label.clone(),
                        description,
                        icon: entry.icon.clone(),
                        actions: vec![NamedAction {
                            label: "Open".into(),
                            action,
                        }],
                    },
                )
            }));
        }
        if self.open_with.is_none() {
            if let Some(batch) = self.batches.get(&1) {
                native.extend(
                    batch
                        .results
                        .iter()
                        .cloned()
                        .map(|result| self.entry(1, result)),
                );
            }
            if matches!(scope, Scope::All | Scope::Settings) {
                native.extend(
                    launcher::controls::results(text, scope == Scope::Settings)
                        .into_iter()
                        .map(|result| self.entry(usize::MAX - 1, result)),
                );
            }
        }
        let mut ranked: Vec<_> = native
            .into_iter()
            .enumerate()
            .map(|(position, entry)| {
                let PanelMenuEntryTarget::Launcher(target) = &entry.target else {
                    unreachable!()
                };
                let mut labels = vec![entry.label.to_lowercase()];
                if let Some(NamedAction {
                    action:
                        Action::FocusWindow {
                            application: Some(id),
                            ..
                        },
                    ..
                }) = target.result.actions.first()
                {
                    labels.push(
                        self.apps
                            .iter()
                            .find(|app| &app.id == id)
                            .map(|app| app.name.clone())
                            .unwrap_or_else(|| id.clone())
                            .to_lowercase(),
                    );
                }
                let query = text.to_lowercase();
                let quality = if query.is_empty() || labels.iter().any(|label| label == &query) {
                    0
                } else if labels.iter().any(|label| label.starts_with(&query)) {
                    1
                } else if labels.iter().any(|label| label.contains(&query)) {
                    2
                } else if super::panel_search::best_match(
                    &[super::panel_search::SearchText {
                        field: super::panel_search::SearchField::Label,
                        text: &entry.label,
                    }],
                    text,
                )
                .is_some()
                {
                    3
                } else {
                    4
                };
                let key = target
                    .result
                    .actions
                    .first()
                    .and_then(|action| launcher::history::key(&action.action));
                let boost = if self.config.history {
                    self.history.boost(key.as_deref())
                } else {
                    0
                };
                (
                    (
                        quality,
                        !self.pinned(&target.result),
                        std::cmp::Reverse(boost),
                        position,
                    ),
                    entry,
                )
            })
            .collect();
        ranked.sort_by_key(|(score, _)| *score);
        let mut seen = std::collections::HashSet::new();
        ranked.retain(|(_, entry)| {
            let PanelMenuEntryTarget::Launcher(target) = &entry.target else {
                return true;
            };
            if matches!(
                target.result.actions.first().map(|a| &a.action),
                Some(Action::FocusWindow { .. })
            ) {
                return true;
            }
            target
                .result
                .actions
                .first()
                .and_then(|a| personalization::saved_target(&a.action))
                .is_none_or(|target| seen.insert(target.key()))
        });
        entries.extend(
            ranked
                .into_iter()
                .take(launcher::RESULT_LIMIT)
                .map(|(_, entry)| entry),
        );
        if self.open_with.is_none() {
            for (provider, batch) in &self.batches {
                if *provider > 1
                    && !self
                        .config
                        .extensions
                        .get(*provider - 2)
                        .is_some_and(|extension| extension.id.starts_with("builtin:"))
                {
                    entries.extend(
                        batch
                            .results
                            .iter()
                            .cloned()
                            .map(|result| self.entry(*provider, result)),
                    );
                }
            }
        }
        entries
    }

    fn entry(&self, provider: usize, mut result: ResultItem) -> PanelMenuEntry {
        self.decorate(&mut result);
        PanelMenuEntry {
            label: result.label.clone(),
            icon: result.icon.clone(),
            path: vec![],
            target: PanelMenuEntryTarget::Launcher(Target {
                session: self.id,
                generation: self.generation,
                revision: self.revision,
                provider,
                result: Arc::new(result),
                thumbnail: None,
            }),
            is_submenu: false,
        }
    }

    pub fn status(&self) -> String {
        if self.action_running {
            return "Applying action…".into();
        }
        if !self.action_error.is_empty() {
            return self.action_error.clone();
        }
        if self.naming.is_some() {
            return "Type a snippet name and press Enter to save".into();
        }
        if self.alias_mode.is_some() {
            return "Type a unique single-word alias, then Enter · empty removes the alias".into();
        }
        if self.actions_mode.is_some() {
            return "Search actions · Enter to apply · Escape clears, then goes back".into();
        }
        if self.query.len() > launcher::MAX_QUERY_BYTES {
            return "Search is limited to 1024 bytes".into();
        }
        let (scope, text) = self.query_scope();
        if matches!(scope, Scope::Clipboard | Scope::Snippets) {
            return self.clipboard_status(scope, text);
        }
        if text.is_empty() {
            match scope {
                Scope::Calculator => return "Enter an arithmetic expression".into(),
                Scope::Files => return "Enter a file or folder name".into(),
                _ => {}
            }
        }
        if self
            .open_with
            .as_ref()
            .is_some_and(|choice| choice.apps.is_none())
        {
            return "Finding applications…".into();
        }
        let mut status: Vec<_> = self
            .batches
            .values()
            .filter(|batch| !batch.status.is_empty())
            .map(|batch| batch.status.clone())
            .collect();
        if scope == Scope::Calculator && !text.is_empty() && !self.batches.contains_key(&0) {
            status.push("Calculating…".into());
        }
        if self.app_results.len() > launcher::RESULT_LIMIT {
            status.push("More application matches; refine your search".into());
        }
        if matches!(scope, Scope::All | Scope::Files) && self.index.is_none() {
            status.push("Indexing file names…".into());
        }
        if !text.is_empty()
            && matches!(scope, Scope::All | Scope::Files)
            && self.index.is_some()
            && !self.batches.contains_key(&1)
        {
            status.push("Searching files…".into());
        }
        for (position, extension) in self.config.extensions.iter().enumerate() {
            if (scope == Scope::Extension(position)
                || scope == Scope::All && extension.global && !text.is_empty())
                && !self.batches.contains_key(&(position + 2))
            {
                status.push(format!("Searching {}…", extension.name));
            }
        }
        if status.is_empty() && self.entries().is_empty() {
            status.push("No matches".into());
        }
        if status.is_empty() && self.query.is_empty() {
            status.push(
                "Search apps, windows and files · = calculator · settings for controls".into(),
            );
        }
        for error in [&self.preferences_error, &self.configuration_error]
            .into_iter()
            .flatten()
        {
            status.push(error.clone());
        }
        if let Some(error) = &self.history_error {
            status.push(error.clone());
        }
        status.join(" · ")
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        if self.in_actions() || matches!(self.query_scope().0, Scope::Clipboard | Scope::Snippets) {
            return iced::Subscription::none();
        }
        if let Some(choice) = &self.open_with {
            return iced::Subscription::run_with(
                ChoiceKey {
                    session: self.id,
                    mode: choice.mode,
                    path: choice.path.clone(),
                    apps: self.apps.clone(),
                },
                choice_stream,
            );
        }
        let mut subscriptions = vec![iced::Subscription::run_with(
            IndexKey {
                session: self.id,
                config: self.config.clone(),
                delay: self
                    .refreshed
                    .map(|time| Duration::from_secs(60).saturating_sub(time.elapsed()))
                    .unwrap_or_default(),
            },
            index_stream,
        )];
        let (scope, text) = self.query_scope();
        if self.query.len() > launcher::MAX_QUERY_BYTES
            || (text.is_empty() && !matches!(scope, Scope::Extension(_)))
        {
            return iced::Subscription::batch(subscriptions);
        }
        let mut providers = Vec::new();
        if matches!(scope, Scope::All | Scope::Calculator) {
            providers.push(0);
        }
        if matches!(scope, Scope::All | Scope::Files) && self.index.is_some() {
            providers.push(1);
        }
        for (position, extension) in self.config.extensions.iter().enumerate() {
            if scope == Scope::Extension(position) || scope == Scope::All && extension.global {
                providers.push(position + 2);
            }
        }
        for provider in providers {
            subscriptions.push(iced::Subscription::run_with(
                QueryKey {
                    session: self.id,
                    generation: self.generation,
                    provider,
                    index_revision: if provider == 1 {
                        self.index_revision
                    } else {
                        0
                    },
                    config: self.config.clone(),
                    query: text.into(),
                    explicit_calculator: scope == Scope::Calculator,
                    explicit_extension: matches!(scope, Scope::Extension(_)),
                    index: self.index.clone(),
                },
                query_stream,
            ));
        }
        iced::Subscription::batch(subscriptions)
    }
}

#[derive(Clone)]
struct ChoiceKey {
    session: u64,
    mode: u64,
    path: std::path::PathBuf,
    apps: Vec<crate::desktop_apps::DesktopApplication>,
}
impl Hash for ChoiceKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.session, self.mode).hash(state);
    }
}
fn choice_stream(
    key: &ChoiceKey,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = Message> + Send + 'static>> {
    let key = key.clone();
    Box::pin(iced::stream::channel(
        1,
        move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            let apps = tokio::task::spawn_blocking(move || {
                launcher::applications::for_file(&key.path, &key.apps)
            })
            .await
            .unwrap_or_default();
            let _ = output
                .send(Message::Panel(PanelEvent::Launcher(
                    Event::ApplicationsForFile {
                        session: key.session,
                        mode: key.mode,
                        apps,
                    },
                )))
                .await;
        },
    ))
}

#[derive(Clone)]
struct IndexKey {
    session: u64,
    config: Arc<Config>,
    delay: Duration,
}
impl Hash for IndexKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.session.hash(state);
    }
}

fn index_stream(
    key: &IndexKey,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = Message> + Send + 'static>> {
    let key = key.clone();
    Box::pin(iced::stream::channel(
        1,
        move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            tokio::time::sleep(key.delay).await;
            loop {
                let config = key.config.clone();
                let cancel = launcher::Cancellation::default();
                let guard = launcher::CancelOnDrop(cancel.clone());
                let index =
                    tokio::task::spawn_blocking(move || launcher::build_index(&config, &cancel))
                        .await
                        .unwrap_or_default();
                drop(guard);
                if output
                    .send(Message::Panel(PanelEvent::Launcher(Event::Indexed {
                        session: key.session,
                        index: Arc::new(index),
                        refreshed: Instant::now(),
                    })))
                    .await
                    .is_err()
                {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        },
    ))
}

#[derive(Clone)]
struct QueryKey {
    session: u64,
    generation: u64,
    provider: usize,
    index_revision: u64,
    config: Arc<Config>,
    query: String,
    explicit_calculator: bool,
    explicit_extension: bool,
    index: Option<Arc<FileIndex>>,
}
impl Hash for QueryKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (
            self.session,
            self.generation,
            self.provider,
            self.index_revision,
        )
            .hash(state);
    }
}

fn query_stream(
    key: &QueryKey,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = Message> + Send + 'static>> {
    let key = key.clone();
    Box::pin(iced::stream::channel(
        1,
        move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let batch = match key.provider {
                0 => launcher::calculate(&key.query, key.explicit_calculator).await,
                1 => {
                    let index = key.index.unwrap_or_default();
                    let query = key.query.clone();
                    let cancel = launcher::Cancellation::default();
                    let _guard = launcher::CancelOnDrop(cancel.clone());
                    tokio::task::spawn_blocking(move || {
                        launcher::search_files(&index, &query, &cancel)
                    })
                    .await
                    .unwrap_or_default()
                }
                provider => {
                    let mut extension = key.config.extensions[provider - 2].clone();
                    if key.explicit_extension {
                        match extension.id.as_str() {
                            "builtin:web" => extension.argv[2] = "web-search".into(),
                            "builtin:date-time" => extension.argv[2] = "date-time-search".into(),
                            _ => {}
                        }
                    }
                    launcher::query_extension(&extension, &key.query).await
                }
            };
            let _ = output
                .send(Message::Panel(PanelEvent::Launcher(Event::Results {
                    session: key.session,
                    generation: key.generation,
                    provider: key.provider,
                    index_revision: key.index_revision,
                    batch,
                })))
                .await;
        },
    ))
}

pub(crate) fn file_uri(path: &std::path::Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut uri = String::from("file://");
    for byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(byte) {
            uri.push(*byte as char);
        } else {
            use std::fmt::Write;
            let _ = write!(uri, "%{byte:02X}");
        }
    }
    uri
}

impl super::App {
    pub(super) fn sync_launcher_context(&mut self) {
        if let Some(active) = self.foreground.launcher()
            && active.preferences_changed(
                &self.launcher_preferences.value,
                &self.launcher_preferences.error,
            )
        {
            self.foreground.launcher_event(Event::Preferences {
                session: active.id,
                value: self.launcher_preferences.value.clone(),
                error: self.launcher_preferences.error.clone(),
            });
        }
        let Some(active) = self.foreground.launcher() else {
            return;
        };
        if active.displays_clipboard()
            && !Arc::ptr_eq(&active.clipboard, &self.launcher_clipboard.view)
        {
            self.foreground.launcher_event(Event::ClipboardChanged {
                session: active.id,
                snapshot: self.launcher_clipboard.view.clone(),
            });
        }
        let Some(active) = self.foreground.launcher() else {
            return;
        };
        if active.tracks_history() {
            self.launcher_history.start();
        }
        let windows = launcher::windows::snapshot(self.niri.state());
        if active.needs_context(
            &windows,
            &self.launcher_history.snapshot,
            &self.launcher_history.error,
        ) {
            let event = Event::Context {
                session: active.id,
                windows,
                history: self.launcher_history.snapshot.clone(),
                error: self.launcher_history.error.clone(),
            };
            self.foreground.launcher_event(event);
        }
    }
    pub(super) fn handle_launcher_history(
        &mut self,
        event: launcher::history::Event,
    ) -> iced::Task<Message> {
        self.launcher_history.apply(&event);
        if let launcher::history::Event::Cleared {
            session,
            generation,
            result,
            ..
        } = event
        {
            return self.handle_launcher_event(Event::ActionFinished {
                session,
                generation,
                result,
            });
        }
        iced::Task::none()
    }
    pub(super) fn handle_launcher_event(&mut self, event: Event) -> iced::Task<Message> {
        if let Event::Results {
            session,
            generation,
            provider,
            batch,
            ..
        } = &event
            && *provider >= 2
            && let Some(active) = self
                .foreground
                .launcher()
                .filter(|a| a.id == *session && a.generation == *generation)
            && let Some(extension) = active.config.extensions.get(*provider - 2)
        {
            let actions: std::collections::BTreeSet<_> = batch
                .results
                .iter()
                .flat_map(|r| r.actions.iter().map(|a| a.label.clone()))
                .collect();
            let old = self
                .launcher_preferences
                .diagnostics
                .entry(extension.id.clone())
                .or_default();
            old.0 = batch.status.clone();
            if !actions.is_empty() {
                old.1 = actions.into_iter().collect();
            }
        }
        if let Event::Edited {
            result: Ok(value), ..
        } = &event
        {
            self.launcher_preferences
                .apply(launcher::preferences::Event(Ok(value.as_ref().clone())));
        }
        let edited = matches!(&event, Event::Edited { .. });
        let naming = matches!(&event, Event::ClipboardFinished {
            session, generation, result: Ok(Some(_)),
        } if self.foreground.launcher().is_some_and(|active| active.id == *session && active.generation == *generation));
        let event = match event {
            Event::SelectionFinished {
                session,
                generation,
                key,
                result,
            } => {
                if result.is_ok()
                    && let Some(key) = key
                {
                    self.launcher_history.record(key);
                }
                Event::ActionFinished {
                    session,
                    generation,
                    result,
                }
            }
            event => event,
        };
        if let Event::Indexed {
            session,
            index,
            refreshed,
        } = &event
            && let Some(active) = self
                .foreground
                .launcher()
                .filter(|active| active.id == *session)
        {
            self.launcher_files = Some(CachedIndex {
                config: active.config.clone(),
                index: index.clone(),
                refreshed: *refreshed,
            });
        }
        let effects = self.foreground.launcher_event(event);
        let task = iced::Task::batch(self.apply_foreground_effects(effects));
        if naming {
            let effects = self
                .foreground
                .handle(super::foreground::ForegroundIntent::Panel(
                    super::foreground::PanelInteraction::RequestSearchFocus,
                ));
            return iced::Task::batch(
                std::iter::once(task).chain(self.apply_foreground_effects(effects)),
            );
        }
        if edited {
            iced::Task::batch([task, super::panel::request_panel_search_focus()])
        } else {
            task
        }
    }

    pub(super) fn seed_launcher_index(&mut self) {
        if let Some(active) = self
            .foreground
            .launcher()
            .filter(|active| active.index.is_none())
            && let Some(cached) = &self.launcher_files
            && cached.config == active.config
        {
            let event = Event::Indexed {
                session: active.id,
                index: cached.index.clone(),
                refreshed: cached.refreshed,
            };
            self.foreground.launcher_event(event);
        }
    }

    pub(super) fn run_launcher_action(
        &mut self,
        target: Target,
        action: usize,
    ) -> iced::Task<Message> {
        let Some(named) = target.result.actions.get(action).cloned() else {
            return iced::Task::none();
        };
        if let Action::Clipboard(command) = named.action {
            return self.run_clipboard_action(target, command);
        }
        if let Action::TogglePin { shortcut } | Action::SaveAlias { shortcut } = &named.action {
            let pin = matches!(named.action, Action::TogglePin { .. });
            let shortcut = shortcut.clone();
            let session = target.session;
            return iced::Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        let path = canopa_launcher::preferences_path()?;
                        canopa_launcher::Preferences::update(&path, |prefs| {
                            prefs.edit_shortcut(shortcut, pin)
                        })
                    })
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r.map(Arc::new).map_err(|e| e.to_string()))
                },
                move |result| {
                    Message::Panel(PanelEvent::Launcher(Event::Edited { session, result }))
                },
            );
        }
        if let Action::Provider { name } = &named.action {
            let Some(tree) = self.foreground.active_panel_tree().cloned() else {
                return iced::Task::none();
            };
            return self.dispatch_panel_action(
                tree.origin_window,
                tree.origin,
                crate::config::PanelAction::OpenProvider(name.clone()),
            );
        }
        if matches!(named.action, Action::ClearHistory) {
            return match self
                .launcher_history
                .clear(target.session, target.generation)
            {
                Ok(Some(event)) => self.handle_launcher_history(event),
                Ok(None) => iced::Task::none(),
                Err(error) => self.handle_launcher_event(Event::ActionFinished {
                    session: target.session,
                    generation: target.generation,
                    result: Err(error),
                }),
            };
        }
        let key = self
            .foreground
            .launcher()
            .filter(|session| session.tracks_history())
            .and_then(|_| launcher::history::key(&named.action));
        let finished = move |result| {
            Message::Panel(PanelEvent::Launcher(Event::SelectionFinished {
                key: key.clone(),
                session: target.session,
                generation: target.generation,
                result,
            }))
        };
        match named.action {
            Action::Copy { text } => {
                iced::clipboard::write(text).chain(iced::Task::done(finished(Ok(()))))
            }
            action => iced::Task::perform(
                async move {
                    let result = match action {
                        Action::OpenUrl { url } => {
                            if !canopa_launcher::valid_url(&url) {
                                Err(std::io::Error::other("Invalid web URL"))
                            } else {
                                crate::command::launch_application(&[
                                    "gio".into(),
                                    "open".into(),
                                    url,
                                ])
                                .await
                                .map(|_| ())
                            }
                        }
                        Action::Application { id } => {
                            #[cfg(not(test))]
                            {
                                let app = tokio::task::spawn_blocking(move || {
                                    crate::desktop_apps::discover_installed_applications()
                                        .unwrap_or_default()
                                        .into_iter()
                                        .find(|app| app.id == id)
                                })
                                .await
                                .ok()
                                .flatten();
                                match app {
                                    Some(app) => {
                                        crate::command::spawn_desktop_application_fire_and_forget(
                                            &app,
                                        )
                                        .await
                                        .map(|_| ())
                                    }
                                    None => Err(std::io::Error::other(
                                        "Application is no longer available",
                                    )),
                                }
                            }
                            #[cfg(test)]
                            {
                                let _ = id;
                                Ok(())
                            }
                        }
                        Action::FocusWindow { id, .. } => crate::command::focus_window(id).await,
                        Action::Reveal { path } => reveal_file(&path).await,
                        Action::Preview { path } => crate::command::launch_application(&[
                            "canopa-files".into(),
                            "--quick-look".into(),
                            file_uri(&path),
                        ])
                        .await
                        .map(|_| ()),
                        Action::OpenWith { path, application } => {
                            open_with_application(&path, &application).await
                        }
                        Action::Open { path } => {
                            let available = tokio::task::spawn_blocking({
                                let path = path.clone();
                                move || path.exists()
                            })
                            .await
                            .unwrap_or(false);
                            if !available {
                                Err(std::io::Error::other(
                                    "File or folder is no longer available",
                                ))
                            } else {
                                crate::command::launch_application(&[
                                    "gio".into(),
                                    "open".into(),
                                    file_uri(&path),
                                ])
                                .await
                                .map(|_| ())
                            }
                        }
                        Action::Launch { argv } => {
                            crate::command::launch_application(&argv).await.map(|_| ())
                        }
                        Action::Clipboard(_)
                        | Action::Copy { .. }
                        | Action::ClearHistory
                        | Action::Provider { .. }
                        | Action::ChooseApplication { .. }
                        | Action::TogglePin { .. }
                        | Action::EditAlias { .. }
                        | Action::SaveAlias { .. } => unreachable!(),
                    };
                    result.map_err(|_| {
                        "Could not apply action; check the application or file is available".into()
                    })
                },
                finished,
            ),
        }
    }
}

async fn reveal_file(path: &std::path::Path) -> std::io::Result<()> {
    tokio::time::timeout(Duration::from_secs(3), async {
        let connection = zbus::Connection::session()
            .await
            .map_err(std::io::Error::other)?;
        let proxy = zbus::Proxy::new(
            &connection,
            "org.freedesktop.FileManager1",
            "/org/freedesktop/FileManager1",
            "org.freedesktop.FileManager1",
        )
        .await
        .map_err(std::io::Error::other)?;
        proxy
            .call::<_, _, ()>("ShowItems", &(vec![file_uri(path)], String::new()))
            .await
            .map_err(std::io::Error::other)
    })
    .await
    .map_err(|_| std::io::Error::other("File manager did not respond"))?
}
async fn open_with_application(path: &std::path::Path, id: &str) -> std::io::Result<()> {
    #[cfg(test)]
    {
        let _ = (path, id);
        Ok(())
    }
    #[cfg(not(test))]
    {
        let id = id.to_string();
        let path = path.to_path_buf();
        let application = tokio::task::spawn_blocking(move || {
            crate::desktop_apps::discover_installed_applications()
                .unwrap_or_default()
                .into_iter()
                .find(|app| app.id == id)
        })
        .await
        .ok()
        .flatten()
        .ok_or_else(|| std::io::Error::other("Application is unavailable"))?;
        crate::command::open_desktop_application(&application, &[file_uri(&path)])
            .await
            .map(|_| ())
    }
}

pub(super) struct CachedIndex {
    config: Arc<Config>,
    index: Arc<FileIndex>,
    refreshed: Instant,
}

#[cfg(test)]
mod tests {
    use super::super::panel_menu_session::{PanelMenuSession, PanelMenuSource, SelectedMenuEntry};
    use super::*;

    fn menu() -> PanelMenuSession {
        PanelMenuSession::open(PanelMenuSource::Launcher {
            prompt: None,
            show_icons: true,
            session: Box::new(Session::new(Config::default(), vec![])),
        })
    }

    fn results(menu: &PanelMenuSession, provider: usize, ids: &[&str]) -> Event {
        let active = menu.launcher().unwrap();
        Event::Results {
            session: active.id,
            generation: active.generation,
            provider,
            index_revision: active.index_revision,
            batch: Batch {
                results: ids
                    .iter()
                    .map(|id| ResultItem {
                        id: (*id).into(),
                        label: (*id).into(),
                        description: String::new(),
                        icon: None,
                        actions: vec![NamedAction {
                            label: "Copy".into(),
                            action: Action::Copy { text: (*id).into() },
                        }],
                    })
                    .collect(),
                status: String::new(),
            },
        }
    }

    #[test]
    fn launcher_discards_superseded_queries_sessions_and_displayed_actions() {
        let mut menu = menu();
        menu.set_search_query("old".into());
        let stale = results(&menu, 1, &["old"]);
        menu.set_search_query("new-fixture".into());
        assert!(!menu.apply_launcher(stale));
        assert!(menu.snapshot().entries.is_empty());
        menu.apply_launcher(results(&menu, 1, &["new"]));
        let PanelMenuEntryTarget::Launcher(target) = menu.snapshot().entries[0].target.clone()
        else {
            panic!()
        };
        menu.apply_launcher(results(&menu, 1, &["changed"]));
        assert_eq!(menu.activate_launcher(target, 0), SelectedMenuEntry::None);
        let mut wrong_session = results(&menu, 1, &["wrong"]);
        if let Event::Results { session, .. } = &mut wrong_session {
            *session += 1;
        }
        assert!(!menu.apply_launcher(wrong_session));
    }

    #[test]
    fn launcher_selection_survives_insertion_and_removal_and_prevents_double_action() {
        let mut menu = menu();
        menu.set_search_query("query".into());
        menu.apply_launcher(results(&menu, 1, &["first", "selected", "last"]));
        menu.focus_index(1);
        menu.apply_launcher(results(&menu, 0, &["calculator"]));
        let snapshot = menu.snapshot();
        assert_eq!(snapshot.focused_index, Some(2));
        assert_eq!(snapshot.entries[2].label, "selected");
        menu.apply_launcher(results(&menu, 1, &["first", "last"]));
        assert_eq!(menu.snapshot().focused_index, Some(2));
        assert!(matches!(
            menu.activate_focused(),
            SelectedMenuEntry::Launcher(..)
        ));
        assert_eq!(menu.activate_focused(), SelectedMenuEntry::None);
        let active = menu.launcher().unwrap();
        let failed = Event::ActionFinished {
            session: active.id,
            generation: active.generation,
            result: Err("Could not copy".into()),
        };
        menu.apply_launcher(failed);
        assert_eq!(menu.launcher().unwrap().status(), "Could not copy");
        assert!(matches!(
            menu.activate_focused(),
            SelectedMenuEntry::Launcher(..)
        ));
    }

    #[test]
    fn launcher_late_best_match_is_selected_until_user_navigates() {
        let mut menu = menu();
        menu.set_search_query("2+2".into());
        menu.apply_launcher(results(&menu, 1, &["file"]));
        menu.apply_launcher(results(&menu, 0, &["calculator"]));
        assert_eq!(menu.snapshot().focused_index, Some(0));
        assert_eq!(menu.snapshot().entries[0].label, "calculator");
        menu.focus_index(1);
        menu.apply_launcher(results(&menu, 0, &["updated-calculator"]));
        assert_eq!(menu.snapshot().focused_index, Some(1));
        assert_eq!(menu.snapshot().entries[1].label, "file");
    }

    #[test]
    fn launcher_scopes_route_only_requested_providers() {
        let config = Config {
            extensions: vec![launcher::Extension {
                id: "custom:Projects".into(),
                name: "Projects".into(),
                keyword: "p".into(),
                global: false,
                argv: vec!["fixture".into()],
            }],
            ..Config::default()
        };
        assert_eq!(
            launcher::scope("p shell", &config),
            (Scope::Extension(0), "shell")
        );
        assert_eq!(
            launcher::scope("files budget", &config),
            (Scope::Files, "budget")
        );
        assert_eq!(
            launcher::scope("= 2+2", &config),
            (Scope::Calculator, "2+2")
        );
        assert_eq!(
            launcher::scope("plain query", &config),
            (Scope::All, "plain query")
        );
    }

    fn application(id: &str, name: &str) -> crate::desktop_apps::DesktopApplication {
        crate::desktop_apps::DesktopApplication {
            id: id.into(),
            name: name.into(),
            generic_name: None,
            comment: None,
            keywords: vec![],
            icon: None,
            desktop_file: format!("/tmp/{id}").into(),
            dbus_activatable: false,
        }
    }
    #[test]
    fn live_window_changes_retire_old_actions_and_preserve_scoped_search() {
        let mut menu = menu();
        menu.set_search_query("windows editor".into());
        let session = menu.launcher().unwrap().id;
        menu.apply_launcher(Event::Context {
            session,
            windows: vec![launcher::windows::Window {
                id: 42,
                title: "Editor notes".into(),
                application: Some("editor".into()),
                workspace: "Work".into(),
            }],
            history: Arc::new(Default::default()),
            error: None,
        });
        let PanelMenuEntryTarget::Launcher(target) = menu.snapshot().entries[0].target.clone()
        else {
            panic!()
        };
        assert!(!menu.launcher().unwrap().status().contains("No matches"));
        menu.apply_launcher(Event::Context {
            session,
            windows: vec![],
            history: Arc::new(Default::default()),
            error: None,
        });
        assert_eq!(menu.activate_launcher(target, 0), SelectedMenuEntry::None);
        assert!(menu.snapshot().entries.is_empty());
    }
    #[test]
    fn frequency_only_breaks_comparable_matches_and_can_be_disabled() {
        let apps = vec![
            application("a.desktop", "Editor A"),
            application("b.desktop", "Editor B"),
        ];
        let mut session = Session::new(Config::default(), apps.clone());
        session.search_changed("Editor");
        let mut runtime = launcher::history::Runtime::default();
        runtime.record("app:b.desktop".into());
        session.apply(Event::Context {
            session: session.id,
            windows: vec![],
            history: runtime.snapshot.clone(),
            error: None,
        });
        assert_eq!(session.entries()[0].label, "Editor B");
        session.search_changed("Editor A");
        assert_eq!(session.entries()[0].label, "Editor A");
        let mut disabled = Session::new(
            Config {
                history: false,
                ..Config::default()
            },
            apps,
        );
        disabled.search_changed("Editor");
        disabled.apply(Event::Context {
            session: disabled.id,
            windows: vec![],
            history: runtime.snapshot,
            error: None,
        });
        assert_eq!(disabled.entries()[0].label, "Editor A");
    }
    #[test]
    fn application_chooser_keeps_exact_path_and_rejects_retired_modes() {
        let mut session = Session::new(Config::default(), vec![]);
        session.search_changed("files original");
        let path = std::path::PathBuf::from("/tmp/file with spaces");
        session.enter_open_with(path.clone());
        session.search_changed("");
        let mode = session.mode_serial;
        session.apply(Event::ApplicationsForFile {
            session: session.id,
            mode,
            apps: vec![application("editor.desktop", "Editor")],
        });
        let PanelMenuEntryTarget::Launcher(target) = session.entries()[0].target.clone() else {
            panic!()
        };
        assert!(
            matches!(&target.result.actions[0].action,Action::OpenWith{path:actual,application} if actual==&path && application=="editor.desktop")
        );
        assert_eq!(session.leave_open_with().as_deref(), Some("files original"));
        assert!(!session.apply(Event::ApplicationsForFile {
            session: session.id,
            mode,
            apps: vec![]
        }));
    }

    #[test]
    fn launcher_file_uris_preserve_bytes_and_cannot_be_opener_options() {
        use std::os::unix::ffi::OsStringExt;
        let path = std::path::PathBuf::from(std::ffi::OsString::from_vec(
            b"/tmp/-odd '?#\n\xff".to_vec(),
        ));
        assert_eq!(file_uri(&path), "file:///tmp/-odd%20%27%3F%23%0A%FF");
    }
}
