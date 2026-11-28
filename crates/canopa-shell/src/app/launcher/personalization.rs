use super::*;
use canopa_launcher::{Preferences, Shortcut, Target as SavedTarget};
#[derive(Clone, Debug)]
pub(super) struct Actions {
    pub result: Arc<ResultItem>,
    pub previous_query: String,
}
#[derive(Clone, Debug)]
pub(super) struct Alias {
    pub shortcut: Shortcut,
    pub previous_query: String,
}
pub(super) fn saved_target(action: &Action) -> Option<SavedTarget> {
    match action {
        Action::Application { id } => Some(SavedTarget::Application { id: id.clone() }),
        Action::FocusWindow {
            application: Some(id),
            ..
        } => Some(SavedTarget::Application { id: id.clone() }),
        Action::Open { path } => Some(SavedTarget::file(path)),
        Action::OpenUrl { url } => Some(SavedTarget::Url { url: url.clone() }),
        Action::Provider { name } => Some(SavedTarget::Control { name: name.clone() }),
        Action::Launch { argv } => Some(SavedTarget::Command { argv: argv.clone() }),
        _ => None,
    }
}
fn action(target: &SavedTarget) -> Action {
    match target {
        SavedTarget::Application { id } => Action::Application { id: id.clone() },
        SavedTarget::File { .. } => Action::Open {
            path: target.path().unwrap(),
        },
        SavedTarget::Url { url } => Action::OpenUrl { url: url.clone() },
        SavedTarget::Control { name } => Action::Provider { name: name.clone() },
        SavedTarget::Command { argv } => Action::Launch { argv: argv.clone() },
    }
}
impl Session {
    pub fn preferences_changed(&self, value: &Arc<Preferences>, error: &Option<String>) -> bool {
        self.preferences.as_ref() != value.as_ref() || &self.preferences_error != error
    }
    pub fn set_preferences(&mut self, value: Arc<Preferences>, error: Option<String>) {
        if self.preferences.as_ref() != value.as_ref()
            && let Some(mode) = self.actions_mode.take()
        {
            self.query = mode.previous_query;
        }
        self.preferences = value;
        self.preferences_error = error;
        let (config, problem) = launcher::bundled::effective(&self.base_config, &self.preferences);
        self.config = Arc::new(config);
        self.configuration_error = problem;
        let query = self.query.clone();
        self.search_changed(&query);
    }
    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn in_child(&self) -> bool {
        self.open_with.is_some() || self.in_actions()
    }
    pub fn in_actions(&self) -> bool {
        self.actions_mode.is_some() || self.alias_mode.is_some() || self.naming.is_some()
    }
    pub fn enter_actions(&mut self, target: Target) -> bool {
        if !self.accepts(&target) || self.in_actions() {
            return false;
        }
        self.actions_mode = Some(Actions {
            result: target.result,
            previous_query: self.query.clone(),
        });
        true
    }
    pub fn enter_alias(&mut self, shortcut: Shortcut) {
        let previous_query = self
            .actions_mode
            .take()
            .map(|m| m.previous_query)
            .unwrap_or_else(|| self.query.clone());
        self.alias_mode = Some(Alias {
            shortcut,
            previous_query,
        });
    }
    pub fn leave_child(&mut self) -> Option<String> {
        self.revision = self.revision.wrapping_add(1);
        self.actions_mode
            .take()
            .map(|m| m.previous_query)
            .or_else(|| self.alias_mode.take().map(|m| m.previous_query))
            .or_else(|| self.naming.take().map(|mode| mode.previous_query))
            .or_else(|| self.leave_open_with())
    }
    pub fn child_entries(&self) -> Option<Vec<PanelMenuEntry>> {
        if let Some(entries) = self.naming_entry() {
            return Some(entries);
        }
        if let Some(mode) = &self.actions_mode {
            use super::super::panel_search::{
                SearchField, SearchMatch, SearchText, best_match, rank_matches,
            };
            let entries = mode
                .result
                .actions
                .iter()
                .enumerate()
                .filter_map(|(i, a)| {
                    let score = best_match(
                        &[SearchText {
                            field: SearchField::Label,
                            text: &a.label,
                        }],
                        &self.query,
                    )?;
                    Some(SearchMatch {
                        score,
                        source_index: i,
                        item: self.entry(
                            usize::MAX - 3,
                            ResultItem {
                                id: format!("action:{i}"),
                                label: a.label.clone(),
                                description: mode.result.label.clone(),
                                icon: None,
                                actions: vec![a.clone()],
                            },
                        ),
                    })
                })
                .collect();
            let entries = rank_matches(entries);
            return Some(entries);
        }
        if let Some(mode) = &self.alias_mode {
            let alias = self.query.trim();
            let valid = alias.is_empty() || canopa_launcher::valid_keyword(alias);
            let mut shortcut = mode.shortcut.clone();
            shortcut.alias = alias.to_owned();
            return Some(if valid {
                vec![self.entry(
                    usize::MAX - 4,
                    ResultItem {
                        id: "save-alias".into(),
                        label: if alias.is_empty() {
                            fl!("shell-launcher-remove-alias")
                        } else {
                            fl!("shell-launcher-save-alias-named", alias = alias.to_string())
                        },
                        description: shortcut.label.clone(),
                        icon: None,
                        actions: vec![NamedAction {
                            label: fl!("shell-launcher-save-alias"),
                            action: Action::SaveAlias { shortcut },
                        }],
                    },
                )]
            } else {
                vec![]
            });
        }
        None
    }
    pub fn shortcut_entries(&self) -> Vec<PanelMenuEntry> {
        let (scope, text) = self.query_scope();
        if scope != Scope::All || self.open_with.is_some() {
            return vec![];
        }
        self.preferences
            .shortcuts
            .iter()
            .filter(|s| {
                if text.is_empty() {
                    s.pinned
                } else {
                    (!s.alias.is_empty() && s.alias.eq_ignore_ascii_case(text))
                        || super::super::panel_search::best_match(
                            &[super::super::panel_search::SearchText {
                                field: super::super::panel_search::SearchField::Label,
                                text: &s.label,
                            }],
                            text,
                        )
                        .is_some()
                }
            })
            .map(|s| {
                self.entry(
                    super::PROVIDER_PINNED,
                    ResultItem {
                        id: s.target.key(),
                        label: if !s.alias.is_empty() && s.alias.eq_ignore_ascii_case(text) {
                            s.alias.clone()
                        } else {
                            s.label.clone()
                        },
                        description: match (s.pinned, s.alias.is_empty()) {
                            (true, true) => fl!("shell-launcher-pinned"),
                            (false, true) => fl!("shell-launcher-shortcut"),
                            (true, false) => {
                                fl!("shell-launcher-pinned-alias", alias = s.alias.clone())
                            }
                            (false, false) => {
                                fl!("shell-launcher-shortcut-alias", alias = s.alias.clone())
                            }
                        },
                        icon: s.icon.clone(),
                        actions: if let Some(path) = s.target.path() {
                            launcher::files::actions(&path, s.icon.as_deref() == Some("folder"))
                        } else {
                            vec![NamedAction {
                                label: fl!("shell-launcher-open"),
                                action: action(&s.target),
                            }]
                        },
                    },
                )
            })
            .collect()
    }
    /// An app row of a running app switches to its window first and keeps a
    /// "New window" action unless the entry has a single main window. Its
    /// `[Desktop Action]` groups follow as further actions.
    fn add_application_actions(&self, result: &mut ResultItem) {
        let Some(NamedAction {
            action: Action::Application { id },
            ..
        }) = result.actions.first()
        else {
            return;
        };
        let Some(app) = self.apps.iter().find(|app| &app.id == id) else {
            return;
        };
        let mut desktop: Vec<_> = app
            .actions
            .iter()
            .map(|action| NamedAction {
                label: action.name.clone(),
                action: Action::AppActionLaunch {
                    application: app.id.clone(),
                    action: action.id.clone(),
                },
            })
            .collect();
        let mut actions = Vec::new();
        match launcher::windows::window_for(&self.windows, &self.apps, app) {
            Some(window) => {
                actions.push(NamedAction {
                    label: fl!("shell-launcher-switch-to"),
                    action: Action::FocusWindow {
                        id: window.id,
                        application: Some(app.id.clone()),
                    },
                });
                if !app.single_main_window {
                    match app
                        .actions
                        .iter()
                        .position(|action| action.id == "new-window")
                    {
                        Some(position) => actions.push(desktop.remove(position)),
                        None => actions.push(NamedAction {
                            label: fl!("shell-launcher-new-window"),
                            action: Action::Application { id: app.id.clone() },
                        }),
                    }
                }
                result.description = fl!("shell-launcher-running-application");
            }
            None => actions.push(result.actions[0].clone()),
        }
        actions.extend(desktop);
        actions.extend(result.actions.drain(1..));
        result.actions = actions;
    }
    pub fn decorate(&self, result: &mut ResultItem) {
        if self.in_child() {
            return;
        }
        if !result.id.starts_with("window:") {
            self.add_application_actions(result);
        }
        let Some(target) = result.actions.first().and_then(|a| saved_target(&a.action)) else {
            return;
        };
        if target.validate().is_err() {
            return;
        }
        if matches!(
            result.actions.first().map(|a| &a.action),
            Some(Action::FocusWindow { .. })
        ) && matches!(&target,SavedTarget::Application{id} if !self.apps.iter().any(|app|&app.id==id))
        {
            return;
        }
        let existing = self
            .preferences
            .shortcuts
            .iter()
            .find(|s| s.target == target);
        let shortcut = existing.cloned().unwrap_or_else(|| Shortcut {
            target,
            label: result.label.clone(),
            alias: String::new(),
            pinned: false,
            icon: result.icon.clone(),
        });
        result.actions.push(NamedAction {
            label: if shortcut.pinned {
                fl!("shell-launcher-unpin")
            } else {
                fl!("shell-launcher-pin")
            },
            action: Action::TogglePin {
                shortcut: shortcut.clone(),
            },
        });
        result.actions.push(NamedAction {
            label: if shortcut.alias.is_empty() {
                fl!("shell-launcher-set-alias")
            } else {
                fl!("shell-launcher-edit-alias")
            },
            action: Action::EditAlias { shortcut },
        });
    }
    pub fn pinned(&self, result: &ResultItem) -> bool {
        result
            .actions
            .first()
            .and_then(|a| saved_target(&a.action))
            .is_some_and(|target| {
                self.preferences
                    .shortcuts
                    .iter()
                    .any(|s| s.target == target && s.pinned)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Session {
        Session::new(
            Config::default(),
            vec![crate::desktop_apps::DesktopApplication {
                id: "editor.desktop".into(),
                name: "Editor".into(),
                generic_name: None,
                comment: None,
                keywords: vec![],
                icon: None,
                desktop_file: "/tmp/editor.desktop".into(),
                dbus_activatable: false,
                ..Default::default()
            }],
        )
    }
    fn first(session: &Session) -> Target {
        let PanelMenuEntryTarget::Launcher(target) = session.entries()[0].target.clone() else {
            panic!()
        };
        target
    }
    #[test]
    fn bundled_instant_answers_precede_fuzzy_applications() {
        let mut session = fixture();
        session.apps[0].name = "Time in Tokyo".into();
        session.search_changed("time in Tokyo");
        let provider = session
            .config
            .extensions
            .iter()
            .position(|e| e.id == "builtin:date-time")
            .unwrap()
            + 2;
        session.apply(Event::Results {
            session: session.id,
            generation: session.generation,
            provider,
            index_revision: 0,
            batch: Batch {
                results: vec![ResultItem {
                    id: "value".into(),
                    label: "2026-11-03 12:00:00 JST (+09:00)".into(),
                    description: "Time in Asia/Tokyo".into(),
                    icon: None,
                    actions: vec![NamedAction {
                        label: "Copy".into(),
                        action: Action::Copy {
                            text: "answer".into(),
                        },
                    }],
                }],
                status: String::new(),
            },
        });
        assert!(matches!(
            first(&session).result.actions[0].action,
            Action::Copy { .. }
        ));
    }
    #[test]
    fn actions_filter_without_changing_the_original_target_and_back_restores_query() {
        let mut session = fixture();
        session.search_changed("apps Editor");
        let target = first(&session);
        assert!(session.enter_actions(target));
        session.search_changed("pin");
        let rows = session.entries();
        assert!(rows.iter().any(|r| r.label == "Pin in Launcher"));
        let target = first(&session);
        assert!(
            matches!(&target.result.actions[0].action,Action::TogglePin{shortcut} if shortcut.target==SavedTarget::Application{id:"editor.desktop".into()})
        );
        assert_eq!(session.leave_child().as_deref(), Some("apps Editor"));
        assert!(!session.accepts(&target));
    }
    #[test]
    fn exact_file_action_precedes_an_earlier_fuzzy_label() {
        let mut session = fixture();
        let result = ResultItem {
            id: "file".into(),
            label: "Notes".into(),
            description: String::new(),
            icon: None,
            actions: vec![
                NamedAction {
                    label: "Open containing folder".into(),
                    action: Action::Reveal {
                        path: "/tmp/notes".into(),
                    },
                },
                NamedAction {
                    label: "Pin".into(),
                    action: Action::TogglePin {
                        shortcut: Shortcut {
                            target: SavedTarget::file(std::path::Path::new("/tmp/notes")),
                            label: "Notes".into(),
                            alias: String::new(),
                            pinned: false,
                            icon: None,
                        },
                    },
                },
            ],
        };
        let PanelMenuEntryTarget::Launcher(target) = session.entry(1, result).target else {
            panic!()
        };
        assert!(session.enter_actions(target));
        session.search_changed("pin");
        assert_eq!(first(&session).result.label, "Pin");
    }
    #[test]
    fn explicit_alias_beats_other_matches_and_keeps_file_bytes_and_file_actions() {
        let mut session = fixture();
        let target = SavedTarget::File {
            bytes: b"/tmp/raw-\xff.txt".to_vec(),
        };
        let mut prefs = Preferences::default();
        prefs.shortcuts.push(Shortcut {
            target: target.clone(),
            label: "Notes".into(),
            alias: "work".into(),
            pinned: true,
            icon: None,
        });
        session.set_preferences(Arc::new(prefs), None);
        session.search_changed("work");
        let row = first(&session);
        assert_eq!(row.result.label, "work");
        assert!(
            matches!(&row.result.actions[0].action,Action::Open{path} if SavedTarget::file(path)==target)
        );
        assert!(
            row.result
                .actions
                .iter()
                .any(|a| matches!(a.action, Action::ChooseApplication { .. }))
        );
    }
    #[test]
    fn disabling_an_extension_retires_query_results_and_actions() {
        let mut session = fixture();
        session.search_changed("web hello");
        let generation = session.generation;
        let mut prefs = Preferences::default();
        prefs.extensions.insert(
            "builtin:web".into(),
            canopa_launcher::ExtensionPreference {
                enabled: false,
                keyword: "web".into(),
            },
        );
        session.set_preferences(Arc::new(prefs), None);
        assert!(session.generation > generation);
        assert!(
            !session
                .config
                .extensions
                .iter()
                .any(|e| e.id.starts_with("builtin:web"))
        );
    }
    #[test]
    fn repeated_window_applications_keep_distinct_window_rows() {
        let mut session = fixture();
        session.windows = (1..=2)
            .map(|id| launcher::windows::Window {
                id,
                title: format!("Editor {id}"),
                application: Some("editor.desktop".into()),
                workspace: "Work".into(),
            })
            .collect();
        session.search_changed("windows Editor");
        assert_eq!(session.entries().len(), 2);
    }
}
