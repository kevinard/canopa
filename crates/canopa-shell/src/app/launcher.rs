//! Adapts bounded search providers to the existing selectable Panel Menu.
mod clipboard;
mod personalization;
use super::{
    Message,
    messages::PanelEvent,
    panel_menu_session::{PanelMenuEntry, PanelMenuEntryTarget},
};
use crate::launcher::{
    self, Action, Batch, Config, FileIndex, NamedAction, ResultItem, Scope, characters,
};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickScope {
    Apps,
    Files,
    Windows,
    Settings,
}

impl QuickScope {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Apps => "Apps",
            Self::Files => "Files",
            Self::Windows => "Windows",
            Self::Settings => "Settings",
        }
    }

    pub(super) fn prefix(self) -> &'static str {
        match self {
            Self::Apps => "apps ",
            Self::Files => "files ",
            Self::Windows => "windows ",
            Self::Settings => "settings ",
        }
    }

    fn scope(self) -> Scope {
        match self {
            Self::Apps => Scope::Apps,
            Self::Files => Scope::Files,
            Self::Windows => Scope::Windows,
            Self::Settings => Scope::Settings,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub session: u64,
    pub generation: u64,
    pub revision: u64,
    pub provider: usize,
    pub result: Arc<ResultItem>,
    pub(super) display_description: Option<Arc<str>>,
    pub(super) display_detail: Option<Arc<str>>,
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
    /// The session rows logind and the configured confirmations offer.
    SessionRows {
        session: u64,
        rows: Arc<Vec<launcher::system::SessionRow>>,
    },
    /// The Settings page registry, or why it is unavailable.
    SettingsPages {
        session: u64,
        pages: Result<Arc<Vec<launcher::system::SettingsPage>>, String>,
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
    input_scope: Option<QuickScope>,
    windows: Vec<launcher::windows::Window>,
    history: Arc<launcher::history::Snapshot>,
    history_error: Option<String>,
    open_with: Option<OpenWith>,
    mode_serial: u64,
    index: Option<Arc<FileIndex>>,
    index_revision: u64,
    refreshed: Option<Instant>,
    batches: BTreeMap<usize, Batch>,
    session_rows: Arc<Vec<launcher::system::SessionRow>>,
    settings_pages: Option<Arc<Vec<launcher::system::SettingsPage>>>,
    pub action_running: bool,
    action_error: String,
}

/// Providers of native rows, for result identity.
const PROVIDER_RECENT: usize = usize::MAX - 6;
const PROVIDER_SESSION: usize = usize::MAX - 7;
const PROVIDER_SETTINGS_PAGES: usize = usize::MAX - 8;
const PROVIDER_DESKTOP_ACTIONS: usize = usize::MAX - 9;
/// Recently launched apps shown above "All apps" in the empty-query list.
const RECENT_LIMIT: usize = 5;
/// The shortest query that lists matching `[Desktop Action]` rows.
const DESKTOP_ACTION_QUERY_CHARS: usize = 3;

/// The folded names that identify an app besides its title: the desktop-file
/// id, its last dotted component and the `Exec` program file name.
fn application_identities(app: &crate::desktop_apps::DesktopApplication) -> Vec<String> {
    use super::panel_search::fold;
    let id = app.id.strip_suffix(".desktop").unwrap_or(&app.id);
    let mut names = vec![fold(id)];
    if let Some((_, last)) = id.rsplit_once('.') {
        names.push(fold(last));
    }
    names.extend(app.exec_program.as_deref().map(fold));
    names
}

pub(super) const APPROXIMATE_KEYWORD_MATCH: &str = "Approximate keyword match";

fn application_match_summary(app: &crate::desktop_apps::DesktopApplication, query: &str) -> String {
    use super::panel_search::{SearchField, SearchText, best_match};

    let name = SearchText {
        field: SearchField::Label,
        text: &app.name,
    };
    if best_match(&[name], query).is_none() {
        let keywords = app.keywords.join(" ");
        let field = SearchText {
            field: SearchField::Secondary,
            text: &keywords,
        };
        if best_match(&[name, field], query).is_none() {
            let identities = application_identities(app);
            return if super::panel_search::query_tokens(query)
                .iter()
                .all(|token| identities.iter().any(|name| name.contains(token)))
            {
                "Matched app file or command name".into()
            } else {
                "Matched app description".into()
            };
        }
        let name = app.name.to_ascii_lowercase();
        let keywords = keywords.to_ascii_lowercase();
        return if super::panel_search::query_tokens(query)
            .iter()
            .all(|token| name.contains(token) || keywords.contains(token))
        {
            "Matched app keywords".into()
        } else {
            APPROXIMATE_KEYWORD_MATCH.into()
        };
    }
    application_summary(app)
}

fn application_summary(app: &crate::desktop_apps::DesktopApplication) -> String {
    let name = app.name.to_lowercase();
    app.generic_name
        .iter()
        .chain(app.comment.iter())
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|text| {
            if let Some(prefix) = text
                .strip_suffix(&app.name)
                .filter(|prefix| prefix.ends_with(' '))
            {
                prefix.trim_end().to_owned()
            } else {
                text
            }
        })
        .find(|text| !text.is_empty() && !text.to_lowercase().contains(&name))
        .unwrap_or_else(|| "Application".into())
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
            input_scope: None,
            windows: vec![],
            history: Arc::new(Default::default()),
            history_error: None,
            open_with: None,
            mode_serial: 0,
            index: None,
            index_revision: 0,
            refreshed: None,
            batches: BTreeMap::new(),
            session_rows: Arc::default(),
            settings_pages: None,
            action_running: false,
            action_error: String::new(),
        }
    }

    pub fn search_changed(&mut self, query: &str) {
        self.query = query.into();
        if !self.in_child()
            && let Some(scope) = self.input_scope
            && launcher::scope(query, &self.config).0 != scope.scope()
        {
            self.input_scope = None;
        }
        self.generation = self.generation.wrapping_add(1);
        self.revision = self.revision.wrapping_add(1);
        self.batches.clear();
        self.action_error.clear();
        let (scope, text) = self.query_scope();
        if let Scope::Characters(kind) = scope {
            if query.len() <= launcher::MAX_QUERY_BYTES {
                let batch = characters::search(kind, text);
                self.batches.insert(characters::PROVIDER, batch);
            }
            self.app_results.clear();
            return;
        }
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
            Event::SessionRows { session, rows } if session == self.id => {
                self.session_rows = rows;
            }
            Event::SettingsPages { session, pages } if session == self.id => {
                self.settings_pages = Some(pages.unwrap_or_default());
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
    pub(super) fn quick_scopes(&self) -> Vec<QuickScope> {
        [
            QuickScope::Apps,
            QuickScope::Files,
            QuickScope::Windows,
            QuickScope::Settings,
        ]
        .into_iter()
        .filter(|scope| launcher::scope(scope.prefix(), &self.config).0 == scope.scope())
        .collect()
    }

    pub(super) fn quick_scope_selection(&self) -> (Option<QuickScope>, &str) {
        let (scope, term) = self.query_scope();
        let selected = match scope {
            Scope::Apps => Some(QuickScope::Apps),
            Scope::Files => Some(QuickScope::Files),
            Scope::Windows => Some(QuickScope::Windows),
            Scope::Settings => Some(QuickScope::Settings),
            _ => None,
        };
        (selected, selected.map_or(self.query.as_str(), |_| term))
    }

    pub(super) fn input_scope(&self) -> Option<QuickScope> {
        self.input_scope.filter(|_| !self.in_child())
    }

    pub(super) fn select_scope(&mut self, scope: Option<QuickScope>) -> Option<String> {
        if self.in_child() || scope.is_some_and(|scope| !self.quick_scopes().contains(&scope)) {
            return None;
        }
        let (_, term) = self.quick_scope_selection();
        let query = format!("{}{}", scope.map_or("", QuickScope::prefix), term);
        self.input_scope = scope;
        Some(query)
    }

    pub(super) fn filter_placeholder(&self) -> Option<&'static str> {
        if self.actions_mode.is_some() {
            Some("Filter actions")
        } else if self.open_with.is_some() {
            Some("Filter applications")
        } else {
            None
        }
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
        if matches!(scope, Scope::Characters(_)) {
            return self
                .batches
                .get(&characters::PROVIDER)
                .into_iter()
                .flat_map(|batch| batch.results.iter().cloned())
                .map(|result| self.entry(characters::PROVIDER, result))
                .collect();
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
        if self.open_with.is_none()
            && matches!(scope, Scope::All | Scope::Apps)
            && super::panel_search::is_blank_query(text)
        {
            entries.extend(self.browse_entries(scope == Scope::All));
            return entries;
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
            native.extend(self.app_results.iter().filter_map(|entry| {
                let PanelMenuEntryTarget::InstalledApp(id) = &entry.target else {
                    return None;
                };
                Some(self.installed_row(entry, text, usize::MAX, id.clone(), None))
            }));
            if self.open_with.is_none() {
                native.extend(self.desktop_action_entries(text));
            }
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
                if let Some(pages) = &self.settings_pages {
                    native.extend(
                        launcher::system::page_results(pages, text, scope == Scope::Settings)
                            .into_iter()
                            .map(|result| self.entry(PROVIDER_SETTINGS_PAGES, result)),
                    );
                }
            }
            if scope == Scope::All {
                native.extend(
                    launcher::system::session_results(&self.session_rows, text)
                        .into_iter()
                        .map(|result| self.entry(PROVIDER_SESSION, result)),
                );
            }
        }
        let query = super::panel_search::fold(text);
        let mut ranked: Vec<_> = native
            .into_iter()
            .enumerate()
            .map(|(position, entry)| {
                let PanelMenuEntryTarget::Launcher(target) = &entry.target else {
                    unreachable!()
                };
                let mut labels = vec![super::panel_search::fold(&entry.label)];
                if !target.result.id.starts_with("window:")
                    && let Some(app) = self.result_application(&target.result)
                {
                    labels.extend(application_identities(app));
                }
                if let Some(NamedAction {
                    action:
                        Action::FocusWindow {
                            application: Some(id),
                            ..
                        },
                    ..
                }) = target.result.actions.first()
                {
                    labels.push(super::panel_search::fold(
                        self.apps
                            .iter()
                            .find(|app| &app.id == id)
                            .map_or(id.as_str(), |app| app.name.as_str()),
                    ));
                }
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
        // In broad search a literal name hit should not be buried among
        // incidental subsequences. Keep literal metadata matches and retain
        // fuzzy fallback for abbreviations and explicitly scoped searches.
        if scope == Scope::All
            && query.chars().count() >= 3
            && ranked.iter().any(|(score, _)| score.0 <= 2)
        {
            let words: Vec<_> = query.split_whitespace().collect();
            ranked.retain(|(score, entry)| {
                if score.0 <= 2 {
                    return true;
                }
                let PanelMenuEntryTarget::Launcher(target) = &entry.target else {
                    return false;
                };
                let mut fields = vec![entry.label.as_str(), target.result.description.as_str()];
                let mut identities = Vec::new();
                if let Some(app) = self.result_application(&target.result) {
                    fields.extend(app.generic_name.as_deref());
                    fields.extend(app.comment.as_deref());
                    fields.extend(app.keywords.iter().map(String::as_str));
                    identities = application_identities(app);
                }
                let mut fields: Vec<_> =
                    fields.into_iter().map(super::panel_search::fold).collect();
                fields.extend(identities);
                words
                    .iter()
                    .all(|word| fields.iter().any(|field| field.contains(word)))
            });
        }
        ranked.sort_by_key(|(score, _)| *score);
        let mut seen = std::collections::HashSet::new();
        ranked.retain(|(_, entry)| {
            let PanelMenuEntryTarget::Launcher(target) = &entry.target else {
                return true;
            };
            // Window rows are distinct even for one app; a running app's row is not.
            if target.result.id.starts_with("window:") {
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

    /// The installed app a native row launches, switches to or acts on.
    fn result_application(
        &self,
        result: &ResultItem,
    ) -> Option<&crate::desktop_apps::DesktopApplication> {
        let id = match &result.actions.first()?.action {
            Action::Application { id }
            | Action::AppActionLaunch {
                application: id, ..
            }
            | Action::FocusWindow {
                application: Some(id),
                ..
            } => id,
            _ => return None,
        };
        self.apps.iter().find(|app| &app.id == id)
    }

    /// One installed-app row. `summary` replaces the match summary.
    fn installed_row(
        &self,
        entry: &PanelMenuEntry,
        text: &str,
        provider: usize,
        id: String,
        summary: Option<&str>,
    ) -> PanelMenuEntry {
        let apps = self
            .open_with
            .as_ref()
            .and_then(|choice| choice.apps.as_deref())
            .unwrap_or(&self.apps);
        let PanelMenuEntryTarget::InstalledApp(app_id) = &entry.target else {
            unreachable!("installed-app rows")
        };
        let action = if let Some(choice) = &self.open_with {
            Action::OpenWith {
                path: choice.path.clone(),
                application: app_id.clone(),
            }
        } else {
            Action::Application { id: app_id.clone() }
        };
        let app = entry.path.first().and_then(|&index| apps.get(index));
        let display_description = summary
            .map(Arc::<str>::from)
            .or_else(|| app.map(|app| Arc::<str>::from(application_match_summary(app, text))));
        let display_detail = app
            .and_then(|app| app.comment.as_deref())
            .map(str::trim)
            .filter(|comment| {
                !comment.is_empty() && display_description.as_deref() != Some(*comment)
            })
            .map(Arc::<str>::from);
        let description = self.open_with.as_ref().map_or_else(
            || "Application".into(),
            |choice| format!("Open with · {}", choice.path.display()),
        );
        self.entry_with_description(
            provider,
            ResultItem {
                id,
                label: entry.label.clone(),
                description,
                icon: entry.icon.clone(),
                actions: vec![NamedAction {
                    label: "Open".into(),
                    action,
                }],
            },
            display_description,
            display_detail,
        )
    }

    /// The empty-query list: pins and recent apps, then every app A–Z.
    fn browse_entries(&self, suggestions: bool) -> Vec<PanelMenuEntry> {
        let mut rows = Vec::new();
        if suggestions {
            rows.extend(self.shortcut_entries());
            let pinned: std::collections::HashSet<_> = self
                .preferences
                .shortcuts
                .iter()
                .filter(|shortcut| shortcut.pinned)
                .filter_map(|shortcut| match &shortcut.target {
                    canopa_launcher::Target::Application { id } => Some(id.as_str()),
                    _ => None,
                })
                .collect();
            let recent = if self.config.history {
                self.history.recent_applications()
            } else {
                Vec::new()
            };
            rows.extend(
                recent
                    .into_iter()
                    .filter(|id| !pinned.contains(id))
                    .filter_map(|id| {
                        self.app_results.iter().find(|entry| {
                            matches!(&entry.target, PanelMenuEntryTarget::InstalledApp(app) if app == id)
                        })
                    })
                    .take(RECENT_LIMIT)
                    .map(|entry| {
                        let PanelMenuEntryTarget::InstalledApp(id) = &entry.target else {
                            unreachable!()
                        };
                        self.installed_row(
                            entry,
                            "",
                            PROVIDER_RECENT,
                            format!("recent:{id}"),
                            Some("Recent"),
                        )
                    }),
            );
        }
        rows.extend(self.app_results.iter().filter_map(|entry| {
            let PanelMenuEntryTarget::InstalledApp(id) = &entry.target else {
                return None;
            };
            Some(self.installed_row(entry, "", usize::MAX, id.clone(), None))
        }));
        rows
    }

    /// "Firefox — New Private Window" rows for `[Desktop Action]` names that
    /// the query names literally ("private", or "firefox private").
    fn desktop_action_entries(&self, text: &str) -> Vec<PanelMenuEntry> {
        use super::panel_search::{fold, query_tokens};
        let tokens = query_tokens(text);
        if tokens
            .iter()
            .map(|token| token.chars().count())
            .sum::<usize>()
            < DESKTOP_ACTION_QUERY_CHARS
        {
            return Vec::new();
        }
        let mut rows = Vec::new();
        for app in &self.apps {
            let app_name = fold(&app.name);
            for action in &app.actions {
                let action_name = fold(&action.name);
                if !tokens.iter().any(|token| action_name.contains(token))
                    || !tokens
                        .iter()
                        .all(|token| action_name.contains(token) || app_name.contains(token))
                {
                    continue;
                }
                let icon = action
                    .icon
                    .iter()
                    .chain(app.icon.iter())
                    .find(|icon| crate::icon_theme::is_valid_icon_reference(icon))
                    .cloned();
                rows.push(self.entry_with_description(
                    PROVIDER_DESKTOP_ACTIONS,
                    ResultItem {
                        id: format!("desktop-action:{}:{}", app.id, action.id),
                        label: format!("{} — {}", app.name, action.name),
                        description: "Application action".into(),
                        icon,
                        actions: vec![NamedAction {
                            label: action.name.clone(),
                            action: Action::AppActionLaunch {
                                application: app.id.clone(),
                                action: action.id.clone(),
                            },
                        }],
                    },
                    Some(Arc::from(format!("{} action", app.name))),
                    None,
                ));
            }
        }
        rows
    }

    fn entry(&self, provider: usize, result: ResultItem) -> PanelMenuEntry {
        self.entry_with_description(provider, result, None, None)
    }

    fn entry_with_description(
        &self,
        provider: usize,
        mut result: ResultItem,
        display_description: Option<Arc<str>>,
        display_detail: Option<Arc<str>>,
    ) -> PanelMenuEntry {
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
                display_description,
                display_detail,
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
            return String::new();
        }
        if self.query.len() > launcher::MAX_QUERY_BYTES {
            return "Search is limited to 1024 bytes".into();
        }
        let (scope, text) = self.query_scope();
        if matches!(scope, Scope::Clipboard | Scope::Snippets) {
            return self.clipboard_status(scope, text);
        }
        if let Some(batch) = self.batches.get(&characters::PROVIDER) {
            return batch.status.clone();
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
            .filter(|batch| {
                !batch.status.is_empty()
                    && (scope == Scope::Files
                        || batch.status != launcher::files::MATCH_LIMIT_STATUS)
            })
            .map(|batch| batch.status.clone())
            .collect();
        if scope == Scope::Calculator && !text.is_empty() && !self.batches.contains_key(&0) {
            status.push("Calculating…".into());
        }
        if !text.is_empty() && self.app_results.len() > launcher::RESULT_LIMIT {
            status.push("Showing the highest-ranked app matches.".into());
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
        if self.in_actions()
            || matches!(
                self.query_scope().0,
                Scope::Clipboard | Scope::Snippets | Scope::Characters(_)
            )
        {
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
        if self.settings_pages.is_none() {
            subscriptions.push(iced::Subscription::run_with(
                PagesKey { session: self.id },
                pages_stream,
            ));
        }
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
                    web_template: self.preferences.web_template.clone(),
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

/// Waits before a query, so a pause in typing starts a search, not each key.
const IN_PROCESS_DEBOUNCE: Duration = Duration::from_millis(100);
const PROCESS_DEBOUNCE: Duration = Duration::from_millis(250);

#[derive(Clone, Hash)]
struct PagesKey {
    session: u64,
}

fn pages_stream(
    key: &PagesKey,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = Message> + Send + 'static>> {
    let session = key.session;
    Box::pin(iced::stream::channel(
        1,
        move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            #[cfg(not(test))]
            let pages = launcher::system::fetch_pages().await.map(Arc::new);
            #[cfg(test)]
            let pages = Err::<Arc<Vec<launcher::system::SettingsPage>>, _>(String::new());
            let _ = output
                .send(Message::Panel(PanelEvent::Launcher(Event::SettingsPages {
                    session,
                    pages,
                })))
                .await;
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
    web_template: String,
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
            // Providers that start a process per query (qalc and configured
            // extensions) wait for a longer pause in typing.
            let spawns = match key.provider {
                0 => true,
                1 => false,
                provider => !key.config.extensions[provider - 2]
                    .id
                    .starts_with("builtin:"),
            };
            tokio::time::sleep(if spawns {
                PROCESS_DEBOUNCE
            } else {
                IN_PROCESS_DEBOUNCE
            })
            .await;
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
                    if extension.id.starts_with("builtin:")
                        && let Some(mode) = extension.argv.get(2).cloned()
                    {
                        let template = extension.argv.get(3).cloned().unwrap_or(key.web_template);
                        let query = key.query.clone();
                        tokio::task::spawn_blocking(move || {
                            launcher::bundled::answer(&mode, &template, &query)
                        })
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or_default()
                    } else {
                        launcher::query_extension(&extension, &key.query).await
                    }
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
        let rows = launcher::system::offered_session_rows(
            &self.config.menus,
            &self.session.capabilities,
            launcher::system::installed_lock_command(),
        );
        if *active.session_rows != rows {
            self.foreground.launcher_event(Event::SessionRows {
                session: active.id,
                rows: Arc::new(rows),
            });
        }
        let Some(active) = self.foreground.launcher() else {
            return;
        };
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
        if let Event::SettingsPages {
            pages: Ok(pages), ..
        } = &event
        {
            self.launcher_settings_pages = Some(pages.clone());
        }
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
            .filter(|active| active.settings_pages.is_none())
            && let Some(pages) = &self.launcher_settings_pages
        {
            let event = Event::SettingsPages {
                session: active.id,
                pages: Ok(pages.clone()),
            };
            self.foreground.launcher_event(event);
        }
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
        if let Action::Panel(action) = &named.action {
            let Some(tree) = self.foreground.active_panel_tree().cloned() else {
                return iced::Task::none();
            };
            // A confirmation menu replaces the launcher; anything else closes it.
            let replaces = matches!(action, crate::config::PanelAction::OpenMenu(_));
            let task = self.dispatch_panel_action(tree.origin_window, tree.origin, action.clone());
            if replaces {
                return task;
            }
            return iced::Task::batch([
                task,
                iced::Task::done(Message::Panel(PanelEvent::Launcher(
                    Event::ActionFinished {
                        session: target.session,
                        generation: target.generation,
                        result: Ok(()),
                    },
                ))),
            ]);
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
        let (request_token, token) = if matches!(
            named.action,
            Action::Application { .. }
                | Action::AppActionLaunch { .. }
                | Action::OpenWith { .. }
                | Action::Reveal { .. }
        ) {
            self.launcher_activation_token()
        } else {
            (iced::Task::none(), None)
        };
        match named.action {
            Action::Copy { text } => {
                iced::clipboard::write(text).chain(iced::Task::done(finished(Ok(()))))
            }
            action => iced::Task::batch([
                request_token,
                iced::Task::perform(
                    async move {
                        let token = receive_token(token).await;
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
                                    match installed_application(id).await {
                                    Some(app) => {
                                        crate::command::spawn_desktop_application_fire_and_forget(
                                            &app, token,
                                        )
                                        .await
                                        .map(|_| ())
                                        .map_err(|error| launch_failure(&app.name, &error))
                                    }
                                    None => Err(unavailable_application()),
                                }
                                }
                                #[cfg(test)]
                                {
                                    let _ = (id, token);
                                    Ok(())
                                }
                            }
                            Action::AppActionLaunch {
                                application,
                                action,
                            } => {
                                #[cfg(not(test))]
                                {
                                    match installed_application(application).await {
                                        Some(app) => {
                                            let name = app
                                                .actions
                                                .iter()
                                                .find(|known| known.id == action)
                                                .map_or_else(
                                                    || app.name.clone(),
                                                    |known| {
                                                        format!("{} — {}", app.name, known.name)
                                                    },
                                                );
                                            crate::command::launch_desktop_action(
                                                &app, &action, token,
                                            )
                                            .await
                                            .map(|_| ())
                                            .map_err(|error| launch_failure(&name, &error))
                                        }
                                        None => Err(unavailable_application()),
                                    }
                                }
                                #[cfg(test)]
                                {
                                    let _ = (application, action, token);
                                    Ok(())
                                }
                            }
                            Action::SettingsPage { id } => {
                                #[cfg(not(test))]
                                {
                                    launcher::system::open_page(&id).await
                                }
                                #[cfg(test)]
                                {
                                    let _ = id;
                                    Ok(())
                                }
                            }
                            Action::FocusWindow { id, .. } => {
                                crate::command::focus_window(id).await
                            }
                            Action::Reveal { path } => reveal_file(&path, token).await,
                            Action::Preview { path } => crate::command::launch_application(&[
                                "canopa-files".into(),
                                "--quick-look".into(),
                                file_uri(&path),
                            ])
                            .await
                            .map(|_| ()),
                            Action::OpenWith { path, application } => {
                                open_with_application(&path, &application, token).await
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
                            | Action::Panel(_)
                            | Action::Provider { .. }
                            | Action::ChooseApplication { .. }
                            | Action::TogglePin { .. }
                            | Action::EditAlias { .. }
                            | Action::SaveAlias { .. } => unreachable!(),
                        };
                        result.map_err(|error| action_error_message(&error))
                    },
                    finished,
                ),
            ]),
        }
    }

    /// Asks the compositor for an activation token for the launcher's
    /// surface. The keyboard serial of the key that chose the result is
    /// recorded, so keyboard-only selections get one too.
    fn launcher_activation_token(
        &self,
    ) -> (
        iced::Task<Message>,
        Option<tokio::sync::oneshot::Receiver<Option<String>>>,
    ) {
        use iced_layershell::actions::{
            ActivationTokenCallback, LayerShellCustomAction, LayerShellCustomActionWithId,
        };
        let Some(window) = self.foreground.active_panel_window() else {
            return (iced::Task::none(), None);
        };
        let (complete, received) = tokio::sync::oneshot::channel();
        let complete = std::sync::Mutex::new(Some(complete));
        let callback = ActivationTokenCallback::new(move |token| {
            if let Some(complete) = complete.lock().ok().and_then(|mut slot| slot.take()) {
                let _ = complete.send(token);
            }
        });
        (
            iced::Task::done(Message::LayerShell(LayerShellCustomActionWithId::new(
                Some(window),
                LayerShellCustomAction::RequestActivationToken {
                    app_id: String::new(),
                    callback,
                },
            ))),
            Some(received),
        )
    }
}

/// Bounded activation tokens arrive within a second, or not at all.
const ACTIVATION_TOKEN_BYTES: usize = 4096;

async fn receive_token(
    receiver: Option<tokio::sync::oneshot::Receiver<Option<String>>>,
) -> Option<String> {
    tokio::time::timeout(Duration::from_secs(1), receiver?)
        .await
        .ok()?
        .ok()
        .flatten()
        .filter(|token| {
            !token.is_empty() && token.len() <= ACTIVATION_TOKEN_BYTES && !token.contains('\0')
        })
}

#[cfg(not(test))]
async fn installed_application(id: String) -> Option<crate::desktop_apps::DesktopApplication> {
    tokio::task::spawn_blocking(move || {
        crate::desktop_apps::discover_installed_applications()
            .unwrap_or_default()
            .into_iter()
            .find(|app| app.id == id)
    })
    .await
    .ok()
    .flatten()
}

#[cfg(not(test))]
fn unavailable_application() -> std::io::Error {
    std::io::Error::other(LaunchFailure("Application is no longer available".into()))
}

/// An action failure whose text the launcher status shows as is.
#[derive(Debug)]
struct LaunchFailure(String);

impl std::fmt::Display for LaunchFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for LaunchFailure {}

/// Names the application and the reason, such as a missing program or
/// terminal found before the handoff, or the compositor refusing the spawn.
fn launch_failure(name: &str, error: &std::io::Error) -> std::io::Error {
    std::io::Error::new(
        error.kind(),
        LaunchFailure(format!("Could not start {name}: {error}")),
    )
}

fn action_error_message(error: &std::io::Error) -> String {
    error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<LaunchFailure>())
        .map_or_else(
            || "Could not apply action; check the application or file is available".into(),
            |failure| failure.0.clone(),
        )
}

/// `ShowItems` with the activation token as its startup id, so an open
/// file manager window is raised.
async fn reveal_file(path: &std::path::Path, token: Option<String>) -> std::io::Result<()> {
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
            .call::<_, _, ()>(
                "ShowItems",
                &(vec![file_uri(path)], token.unwrap_or_default()),
            )
            .await
            .map_err(std::io::Error::other)
    })
    .await
    .map_err(|_| std::io::Error::other("File manager did not respond"))?
}
async fn open_with_application(
    path: &std::path::Path,
    id: &str,
    token: Option<String>,
) -> std::io::Result<()> {
    #[cfg(test)]
    {
        let _ = (path, id, token);
        Ok(())
    }
    #[cfg(not(test))]
    {
        let application = installed_application(id.to_owned())
            .await
            .ok_or_else(unavailable_application)?;
        crate::command::open_desktop_application(&application, &[file_uri(path)], token)
            .await
            .map(|_| ())
            .map_err(|error| launch_failure(&application.name, &error))
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
    fn clicked_launcher_scope_preserves_the_query_across_child_modes() {
        let mut session = Session::new(Config::default(), vec![]);
        session.search_changed("files budget");
        assert_eq!(session.input_scope(), None);

        let scoped = session.select_scope(Some(QuickScope::Apps)).unwrap();
        assert_eq!(scoped, "apps budget");
        session.search_changed(&scoped);
        assert_eq!(session.input_scope(), Some(QuickScope::Apps));

        session.enter_open_with(std::path::PathBuf::from("/fixture/budget"));
        session.search_changed("");
        assert_eq!(session.input_scope(), None);
        let restored = session.leave_open_with().unwrap();
        session.search_changed(&restored);
        assert_eq!(session.input_scope(), Some(QuickScope::Apps));
        assert_eq!(session.select_scope(None).as_deref(), Some("budget"));
        assert_eq!(session.input_scope(), None);
    }

    #[test]
    fn launcher_scopes_route_only_requested_providers() {
        let mut config = Config {
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
            launcher::scope("emoji", &config),
            (Scope::Characters(characters::Kind::Emoji), "")
        );
        assert_eq!(
            launcher::scope("symbol em dash", &config),
            (Scope::Characters(characters::Kind::Symbol), "em dash")
        );
        config.extensions[0].keyword = "emoji".into();
        assert_eq!(
            launcher::scope("emoji shell", &config),
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
            ..Default::default()
        }
    }

    #[test]
    fn broad_search_prefers_literal_names_and_keywords_to_incidental_subsequences() {
        let mut keyword_match = application("keywords", "Keyword Manager");
        keyword_match.keywords.push("fire".into());
        let mut session = Session::new(
            Config::default(),
            vec![
                application("firefox", "Firefox"),
                application("renderer", "File Renderer"),
                keyword_match,
            ],
        );
        session.search_changed("fire");
        let entries = session.entries();
        assert_eq!(entries[0].label, "Firefox");
        assert!(entries.iter().any(|entry| entry.label == "Keyword Manager"));
        assert!(!entries.iter().any(|entry| entry.label == "File Renderer"));

        session.search_changed("frend");
        assert!(
            session
                .entries()
                .iter()
                .any(|entry| entry.label == "File Renderer")
        );
    }

    #[test]
    fn launcher_finds_and_ranks_non_latin_and_accented_names() {
        let mut keyword_match = application("party.desktop", "Party Planner");
        keyword_match.keywords.push("Piñata".into());
        let mut session = Session::new(
            Config::default(),
            vec![
                application("cafeteria.desktop", "Cafeteria Menu"),
                application("cafe.desktop", "Café"),
                application("terminal.desktop", "Терминал"),
                application("files.desktop", "Файлы"),
                application("settings.desktop", "Ρυθμίσεις"),
                application("cjk.desktop", "终端"),
                keyword_match,
            ],
        );
        let first = |session: &mut Session, query: &str| {
            session.search_changed(query);
            session.entries().first().map(|entry| entry.label.clone())
        };

        assert_eq!(first(&mut session, "терм").as_deref(), Some("Терминал"));
        assert_eq!(first(&mut session, "ТЕРМИНАЛ").as_deref(), Some("Терминал"));
        assert!(!session.entries().iter().any(|entry| entry.label == "Файлы"));
        // A folded exact match outranks a longer prefix match.
        assert_eq!(first(&mut session, "cafe").as_deref(), Some("Café"));
        assert_eq!(first(&mut session, "CAFÉ").as_deref(), Some("Café"));
        assert_eq!(
            first(&mut session, "ρυθμισεις").as_deref(),
            Some("Ρυθμίσεις")
        );
        assert_eq!(first(&mut session, "终端").as_deref(), Some("终端"));
        assert_eq!(
            first(&mut session, "pinata").as_deref(),
            Some("Party Planner")
        );
    }

    #[test]
    fn launcher_launch_failures_name_the_application_and_reason() {
        let problem = std::io::Error::new(
            std::io::ErrorKind::NotFound,
            crate::desktop_apps::LaunchProblem::MissingProgram("does-not-exist".into()),
        );
        assert_eq!(
            action_error_message(&launch_failure("Broken", &problem)),
            "Could not start Broken: “does-not-exist” is not installed"
        );
        let terminal = std::io::Error::new(
            std::io::ErrorKind::NotFound,
            crate::desktop_apps::LaunchProblem::NoTerminal,
        );
        assert!(
            action_error_message(&launch_failure("htop", &terminal))
                .starts_with("Could not start htop: it runs in a terminal")
        );
        let rejected = std::io::Error::other("niri rejected application launch");
        assert_eq!(
            action_error_message(&launch_failure("Firefox", &rejected)),
            "Could not start Firefox: niri rejected application launch"
        );
        assert_eq!(
            action_error_message(&std::io::Error::other("Invalid web URL")),
            "Could not apply action; check the application or file is available"
        );
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

    fn firefox() -> crate::desktop_apps::DesktopApplication {
        let mut app = application("firefox.desktop", "Firefox");
        app.exec_program = Some("firefox".into());
        app.actions = vec![
            crate::desktop_apps::DesktopAction {
                id: "new-window".into(),
                name: "New Window".into(),
                icon: None,
                exec: Some("firefox --new-window".into()),
            },
            crate::desktop_apps::DesktopAction {
                id: "new-private-window".into(),
                name: "New Private Window".into(),
                icon: None,
                exec: Some("firefox --private-window".into()),
            },
        ];
        app
    }

    fn primary(entry: &PanelMenuEntry) -> &Action {
        let PanelMenuEntryTarget::Launcher(target) = &entry.target else {
            panic!("launcher row")
        };
        &target.result.actions[0].action
    }

    fn action_labels(entry: &PanelMenuEntry) -> Vec<String> {
        let PanelMenuEntryTarget::Launcher(target) = &entry.target else {
            panic!("launcher row")
        };
        target
            .result
            .actions
            .iter()
            .map(|a| a.label.clone())
            .collect()
    }

    #[test]
    fn desktop_actions_are_app_actions_and_rows_when_the_query_names_them() {
        let mut session = Session::new(
            Config::default(),
            vec![firefox(), application("editor.desktop", "Private Notes")],
        );
        session.search_changed("firefox");
        let entries = session.entries();
        assert_eq!(entries[0].label, "Firefox");
        assert!(!entries.iter().any(|entry| entry.label.contains('—')));
        assert_eq!(
            &action_labels(&entries[0])[..3],
            ["Open", "New Window", "New Private Window"]
        );

        session.search_changed("private");
        let labels: Vec<_> = session.entries().iter().map(|e| e.label.clone()).collect();
        assert!(
            labels.contains(&"Firefox — New Private Window".to_owned()),
            "{labels:?}"
        );
        // An app whose own name matches still ranks first.
        assert_eq!(labels[0], "Private Notes");
        let row = session
            .entries()
            .into_iter()
            .find(|entry| entry.label == "Firefox — New Private Window")
            .unwrap();
        assert!(matches!(
            primary(&row),
            Action::AppActionLaunch { application, action }
                if application == "firefox.desktop" && action == "new-private-window"
        ));

        session.search_changed("firefox private");
        assert_eq!(session.entries()[0].label, "Firefox — New Private Window");
        // Short queries do not list every action that contains a letter.
        session.search_changed("ne");
        assert!(
            !session
                .entries()
                .iter()
                .any(|entry| entry.label.contains('—'))
        );
    }

    #[test]
    fn running_apps_switch_to_their_window_and_keep_a_new_window() {
        let mut single = application("org.example.Single.desktop", "Single");
        single.single_main_window = true;
        let mut session = Session::new(Config::default(), vec![firefox(), single]);
        let window = |id, app: &str| launcher::windows::Window {
            id,
            title: "Window".into(),
            application: Some(app.into()),
            workspace: "Workspace 1".into(),
        };
        session.apply(Event::Context {
            session: session.id,
            windows: vec![window(7, "firefox"), window(9, "org.example.Single")],
            history: Arc::new(Default::default()),
            error: None,
        });
        session.search_changed("apps firefox");
        let row = session.entries().remove(0);
        assert!(matches!(
            primary(&row),
            Action::FocusWindow { id: 7, application: Some(app) } if app == "firefox.desktop"
        ));
        assert_eq!(
            &action_labels(&row)[..3],
            ["Switch to", "New Window", "New Private Window"]
        );
        session.search_changed("apps single");
        let row = session.entries().remove(0);
        assert_eq!(action_labels(&row)[0], "Switch to");
        assert!(
            !action_labels(&row)
                .iter()
                .any(|label| label == "New window")
        );
    }

    #[test]
    fn empty_query_browses_pins_recents_and_every_app_alphabetically() {
        let apps: Vec<_> = (0..30)
            .map(|index| {
                application(
                    &format!("app{index:02}.desktop"),
                    &format!("App {index:02}"),
                )
            })
            .collect();
        let mut session = Session::new(Config::default(), apps);
        let mut runtime = launcher::history::Runtime::default();
        runtime.record("app:app07.desktop".into());
        runtime.record("app:app03.desktop".into());
        runtime.record("app:gone.desktop".into());
        session.apply(Event::Context {
            session: session.id,
            windows: vec![],
            history: runtime.snapshot.clone(),
            error: None,
        });
        let mut preferences = canopa_launcher::Preferences::default();
        preferences.shortcuts.push(canopa_launcher::Shortcut {
            target: canopa_launcher::Target::Application {
                id: "app07.desktop".into(),
            },
            label: "App 07".into(),
            alias: String::new(),
            pinned: true,
            icon: None,
        });
        session.set_preferences(Arc::new(preferences), None);
        session.search_changed("");
        let entries = session.entries();
        // One pin, one recent (the pinned app is not repeated), then all 30.
        assert_eq!(entries.len(), 32);
        assert_eq!(entries[0].label, "App 07");
        let PanelMenuEntryTarget::Launcher(recent) = &entries[1].target else {
            panic!()
        };
        assert_eq!(entries[1].label, "App 03");
        assert_eq!(recent.display_description.as_deref(), Some("Recent"));
        let names: Vec<_> = entries[2..].iter().map(|e| e.label.clone()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        assert_eq!(names.len(), 30);

        session.search_changed("apps ");
        assert_eq!(session.entries().len(), 30);
        // A search still shows the best 20.
        session.search_changed("app");
        assert_eq!(session.entries().len(), launcher::RESULT_LIMIT);
    }

    #[test]
    fn apps_match_their_desktop_file_id_and_exec_program() {
        let mut files = application("org.gnome.Nautilus.desktop", "Files");
        files.exec_program = Some("nautilus".into());
        let mut code = application("code-oss.desktop", "Visual Studio Code");
        code.exec_program = Some("code-oss".into());
        let mut session = Session::new(
            Config::default(),
            vec![files, code, application("notes.desktop", "Notes")],
        );
        session.search_changed("nautilus");
        let entries = session.entries();
        assert_eq!(entries[0].label, "Files");
        let PanelMenuEntryTarget::Launcher(target) = &entries[0].target else {
            panic!()
        };
        assert_eq!(
            target.display_description.as_deref(),
            Some("Matched app file or command name")
        );
        session.search_changed("code-oss");
        assert_eq!(session.entries()[0].label, "Visual Studio Code");
    }

    #[test]
    fn session_rows_and_settings_pages_are_searchable() {
        use crate::config::PanelAction;
        let mut session = Session::new(Config::default(), vec![]);
        session.apply(Event::SessionRows {
            session: session.id,
            rows: Arc::new(vec![
                launcher::system::SessionRow {
                    command: launcher::system::SessionCommand::Suspend,
                    action: PanelAction::Session(crate::session::SessionAction::Suspend),
                },
                launcher::system::SessionRow {
                    command: launcher::system::SessionCommand::PowerOff,
                    action: PanelAction::OpenMenu("Power off?".into()),
                },
            ]),
        });
        session.search_changed("shut down");
        let entries = session.entries();
        assert_eq!(entries[0].label, "Power off…");
        assert!(
            matches!(primary(&entries[0]), Action::Panel(PanelAction::OpenMenu(name)) if name == "Power off?")
        );
        session.search_changed("suspend");
        assert_eq!(session.entries()[0].label, "Suspend");
        // Session rows never appear unasked.
        session.search_changed("");
        assert!(session.entries().is_empty());

        session.apply(Event::SettingsPages {
            session: session.id,
            pages: Ok(Arc::new(vec![launcher::system::SettingsPage {
                id: "night-light".into(),
                title: "Night light".into(),
                description: String::new(),
                icon: None,
                keywords: vec!["blue light".into()],
            }])),
        });
        session.search_changed("blue light");
        assert!(matches!(
            primary(&session.entries()[0]),
            Action::SettingsPage { id } if id == "night-light"
        ));
        session.search_changed("settings ");
        assert!(
            session
                .entries()
                .iter()
                .any(|entry| entry.label == "Night light settings")
        );
    }
}
