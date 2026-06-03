use super::panel_search::{self, SearchField, SearchMatch, SearchText};
use std::cell::RefCell;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SelectedMenuEntry {
    ConfiguredPath(Vec<usize>),
    InstalledApp(String),
    DmenuValue(String),
    CustomValue(String),
    SniMenuItem(String),
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PanelMenuEntryTarget {
    StaticPath(Vec<usize>),
    InstalledApp(String),
    DmenuValue(String),
    DmenuCustomValue(String),
    SniMenuItem(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PanelMenuEntry {
    pub(super) label: String,
    pub(super) icon: Option<String>,
    pub(super) path: Vec<usize>,
    pub(super) target: PanelMenuEntryTarget,
    pub(super) is_submenu: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PanelMenuSnapshot {
    pub(super) prompt: Option<String>,
    pub(super) search_query: String,
    pub(super) focused_index: Option<usize>,
    pub(super) show_icons: bool,
    pub(super) search_enabled: bool,
    pub(super) hovered_index: Option<usize>,
    pub(super) entries: Arc<[PanelMenuEntry]>,
}

#[derive(Debug, Clone)]
pub(super) enum PanelMenuSource {
    Configured(crate::config::MenuConfig),
    InstalledApps {
        prompt: Option<String>,
        show_icons: bool,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
    },
    Dmenu(crate::ipc::protocol::DmenuRequest),
    SniMenu {
        tree: crate::surface::menu_popup::MenuTree,
        search_enabled: bool,
    },
}

#[derive(Debug, Clone)]
pub(super) struct PanelMenuSession {
    source: PanelMenuSource,
    search_query: String,
    focused_index: Option<usize>,
    hovered_index: Option<usize>,
    menu_path: Vec<usize>,
    entries_cache: RefCell<Option<(String, Arc<[PanelMenuEntry]>)>>,
}

impl PanelMenuSession {
    pub(super) fn open(source: PanelMenuSource) -> Self {
        let focused_index = if matches!(source, PanelMenuSource::SniMenu { .. }) {
            None
        } else {
            Some(0)
        };
        Self {
            source,
            search_query: String::new(),
            focused_index,
            hovered_index: None,
            menu_path: Vec::new(),
            entries_cache: RefCell::new(None),
        }
    }

    pub(super) fn set_search_query(&mut self, query: String) {
        self.search_query = query;
        self.focused_index = Some(0);
        self.clear_entries_cache();
    }

    pub(super) fn clear_search(&mut self) -> bool {
        if self.search_query.is_empty() {
            return false;
        }
        self.search_query.clear();
        self.focused_index = Some(0);
        self.clear_entries_cache();
        true
    }

    pub(super) fn focus_index(&mut self, index: usize) {
        self.focused_index = Some(index);
    }

    pub(super) fn hover_index(&mut self, index: usize) {
        self.hovered_index = Some(index);
    }

    pub(super) fn clear_hover(&mut self) {
        self.hovered_index = None;
    }

    pub(super) fn focused_index(&self) -> Option<usize> {
        self.focused_index
    }

    pub(super) fn move_focus(&mut self, delta: isize) {
        let entries = self.entries();
        if entries.is_empty() {
            self.focused_index = None;
            return;
        }
        let current = self.focused_index.unwrap_or(0).min(entries.len() - 1);
        let next = if self.focused_index.is_none() && delta > 0 {
            0
        } else if self.focused_index.is_none() {
            entries.len() - 1
        } else if delta > 0 {
            (current + 1) % entries.len()
        } else if current == 0 {
            entries.len() - 1
        } else {
            current - 1
        };
        self.focused_index = Some(next);
    }

    pub(super) fn back(&mut self) -> bool {
        if self.clear_search() {
            return true;
        }
        if self.menu_path.is_empty() {
            return false;
        }
        self.menu_path.pop();
        self.focused_index = Some(0);
        self.clear_entries_cache();
        true
    }

    pub(super) fn snapshot(&self) -> PanelMenuSnapshot {
        PanelMenuSnapshot {
            prompt: self.prompt(),
            search_query: self.search_query.clone(),
            focused_index: self.focused_index,
            show_icons: self.show_icons(),
            search_enabled: self.search_enabled(),
            hovered_index: self.hovered_index,
            entries: self.entries(),
        }
    }

    pub(super) fn activate_focused(&mut self) -> SelectedMenuEntry {
        let Some(index) = self.focused_index else {
            return SelectedMenuEntry::None;
        };
        self.activate_index(index)
    }

    pub(super) fn activate_index(&mut self, index: usize) -> SelectedMenuEntry {
        let Some(entry) = self.entries().get(index).cloned() else {
            return SelectedMenuEntry::None;
        };

        match entry.target {
            PanelMenuEntryTarget::StaticPath(path) => self.activate_static_path(path),
            PanelMenuEntryTarget::InstalledApp(id) => SelectedMenuEntry::InstalledApp(id),
            PanelMenuEntryTarget::DmenuValue(value) => SelectedMenuEntry::DmenuValue(value),
            PanelMenuEntryTarget::DmenuCustomValue(value) => SelectedMenuEntry::CustomValue(value),
            PanelMenuEntryTarget::SniMenuItem(item_id) => SelectedMenuEntry::SniMenuItem(item_id),
        }
    }

    pub(super) fn configured_action_for_path(
        &self,
        path: &[usize],
    ) -> Option<crate::config::PanelAction> {
        let PanelMenuSource::Configured(menu) = &self.source else {
            return None;
        };
        menu_item_at_path(&menu.items, path).and_then(|item| item.action.clone())
    }

    #[cfg(test)]
    pub(super) fn focus_sni_item(&mut self, item_id: &str) {
        if let Some(index) = self.index_for_sni_item(item_id) {
            self.focus_index(index);
        }
    }

    pub(super) fn hover_sni_item(&mut self, item_id: &str) {
        if let Some(index) = self.index_for_sni_item(item_id) {
            self.hover_index(index);
        }
    }

    pub(super) fn focused_sni_item(&self) -> Option<String> {
        self.sni_item_at_index(self.focused_index?)
    }

    pub(super) fn hovered_sni_item(&self) -> Option<String> {
        self.sni_item_at_index(self.hovered_index?)
    }

    fn index_for_sni_item(&self, item_id: &str) -> Option<usize> {
        self.entries().iter().position(
            |entry| matches!(&entry.target, PanelMenuEntryTarget::SniMenuItem(id) if id == item_id),
        )
    }

    fn sni_item_at_index(&self, index: usize) -> Option<String> {
        match self.entries().get(index).map(|entry| &entry.target) {
            Some(PanelMenuEntryTarget::SniMenuItem(id)) => Some(id.clone()),
            _ => None,
        }
    }

    fn prompt(&self) -> Option<String> {
        match &self.source {
            PanelMenuSource::Configured(menu) => menu.prompt.clone(),
            PanelMenuSource::InstalledApps { prompt, .. } => prompt.clone(),
            PanelMenuSource::Dmenu(request) => request.prompt.clone(),
            PanelMenuSource::SniMenu { .. } => None,
        }
    }

    fn show_icons(&self) -> bool {
        match &self.source {
            PanelMenuSource::Configured(menu) => menu.show_icons,
            PanelMenuSource::InstalledApps { show_icons, .. } => *show_icons,
            PanelMenuSource::Dmenu(request) => dmenu_request_show_icons(request),
            PanelMenuSource::SniMenu { .. } => false,
        }
    }

    pub(super) fn search_enabled(&self) -> bool {
        match &self.source {
            PanelMenuSource::SniMenu { search_enabled, .. } => *search_enabled,
            PanelMenuSource::Configured(_)
            | PanelMenuSource::InstalledApps { .. }
            | PanelMenuSource::Dmenu(_) => true,
        }
    }

    pub(super) fn search_query(&self) -> &str {
        &self.search_query
    }

    pub(super) fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    pub(super) fn entries_snapshot(&self) -> Arc<[PanelMenuEntry]> {
        self.entries()
    }

    fn entries(&self) -> Arc<[PanelMenuEntry]> {
        let cache_key = format!("{}:{:?}", self.search_query, self.menu_path);
        if let Some((cached_key, entries)) = self.entries_cache.borrow().as_ref()
            && cached_key == &cache_key
        {
            return entries.clone();
        }
        let entries: Arc<[PanelMenuEntry]> = match &self.source {
            PanelMenuSource::Configured(menu) if self.search_query.is_empty() => {
                current_menu_entries(menu, &self.menu_path).into()
            }
            PanelMenuSource::Configured(menu) => {
                search_menu_entries(menu, &self.search_query).into()
            }
            PanelMenuSource::InstalledApps { apps, .. } => {
                installed_app_entries(apps, &self.search_query).into()
            }
            PanelMenuSource::Dmenu(request) => {
                dmenu_entry_views(request, &self.search_query).into()
            }
            PanelMenuSource::SniMenu {
                tree,
                search_enabled,
            } if *search_enabled && !panel_search::is_blank_query(&self.search_query) => {
                search_sni_menu_entries(tree, &self.search_query).into()
            }
            PanelMenuSource::SniMenu { tree, .. } => sni_menu_entries(tree).into(),
        };
        *self.entries_cache.borrow_mut() = Some((cache_key, entries.clone()));
        entries
    }

    fn activate_static_path(&mut self, path: Vec<usize>) -> SelectedMenuEntry {
        let PanelMenuSource::Configured(menu) = &self.source else {
            return SelectedMenuEntry::None;
        };
        let Some(item) = menu_item_at_path(&menu.items, &path) else {
            return SelectedMenuEntry::None;
        };
        if !item.children.is_empty() && self.search_query.is_empty() {
            self.menu_path = path;
            self.focused_index = Some(0);
            self.clear_entries_cache();
            return SelectedMenuEntry::None;
        }
        if item.action.is_some() {
            SelectedMenuEntry::ConfiguredPath(path)
        } else {
            SelectedMenuEntry::None
        }
    }

    fn clear_entries_cache(&self) {
        self.entries_cache.borrow_mut().take();
    }
}

pub(super) fn dmenu_entry_views(
    request: &crate::ipc::protocol::DmenuRequest,
    query: &str,
) -> Vec<PanelMenuEntry> {
    let exact_match = request
        .entries
        .iter()
        .any(|entry| entry.display_label() == query || entry.selected_value() == query);

    let mut matches = Vec::new();
    for (index, entry) in request.entries.iter().enumerate() {
        let view = PanelMenuEntry {
            label: entry.display_label().to_string(),
            icon: entry.icon.clone(),
            path: vec![index],
            target: PanelMenuEntryTarget::DmenuValue(entry.selected_value().to_string()),
            is_submenu: false,
        };

        if panel_search::is_blank_query(query) {
            matches.push(SearchMatch {
                item: view,
                score: panel_search::best_match(
                    &[SearchText {
                        field: SearchField::Label,
                        text: entry.display_label(),
                    }],
                    "",
                )
                .expect("blank query matches"),
                source_index: index,
            });
            continue;
        }

        let keywords = entry.keywords.join(" ");
        let fields = [
            SearchText {
                field: SearchField::Label,
                text: entry.display_label(),
            },
            SearchText {
                field: SearchField::Secondary,
                text: keywords.as_str(),
            },
        ];

        if let Some(score) = panel_search::best_match(&fields, query) {
            matches.push(SearchMatch {
                item: view,
                score,
                source_index: index,
            });
        }
    }

    let mut entries = if panel_search::is_blank_query(query) {
        matches.into_iter().map(|matched| matched.item).collect()
    } else {
        panel_search::rank_matches(matches)
    };

    if request.allow_custom && !panel_search::is_blank_query(query) && !exact_match {
        entries.push(PanelMenuEntry {
            label: query.to_string(),
            icon: None,
            path: Vec::new(),
            target: PanelMenuEntryTarget::DmenuCustomValue(query.to_string()),
            is_submenu: false,
        });
    }

    entries
}

pub(super) fn dmenu_request_show_icons(request: &crate::ipc::protocol::DmenuRequest) -> bool {
    request.entries.iter().any(|entry| entry.icon.is_some())
}

fn sni_menu_entries(tree: &crate::surface::menu_popup::MenuTree) -> Vec<PanelMenuEntry> {
    tree.items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.visible && item.enabled && !item.is_separator)
        .map(|(index, item)| PanelMenuEntry {
            label: item.label.clone(),
            icon: item.icon_name.clone(),
            path: vec![index],
            target: PanelMenuEntryTarget::SniMenuItem(item.id.clone()),
            is_submenu: !item.children.is_empty(),
        })
        .collect()
}

fn search_sni_menu_entries(
    tree: &crate::surface::menu_popup::MenuTree,
    query: &str,
) -> Vec<PanelMenuEntry> {
    let mut results = Vec::new();
    collect_search_sni_entries(
        &tree.items,
        query,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut 0,
        &mut results,
    );
    panel_search::rank_matches(results)
}

fn collect_search_sni_entries(
    items: &[crate::surface::menu_popup::MenuItem],
    query: &str,
    labels: &mut Vec<String>,
    path: &mut Vec<usize>,
    source_index: &mut usize,
    results: &mut Vec<SearchMatch<PanelMenuEntry>>,
) {
    for (index, item) in items.iter().enumerate() {
        if !item.visible {
            continue;
        }
        labels.push(item.label.clone());
        path.push(index);

        if item.enabled && !item.is_separator {
            let current_source_index = *source_index;
            *source_index += 1;
            let breadcrumb = labels
                .iter()
                .take(labels.len().saturating_sub(1))
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(" / ");
            let fields = [
                SearchText {
                    field: SearchField::Label,
                    text: item.label.as_str(),
                },
                SearchText {
                    field: SearchField::Breadcrumb,
                    text: breadcrumb.as_str(),
                },
            ];
            if let Some(score) = panel_search::best_match(&fields, query) {
                results.push(SearchMatch {
                    item: PanelMenuEntry {
                        label: labels.join(" / "),
                        icon: item.icon_name.clone(),
                        path: path.clone(),
                        target: PanelMenuEntryTarget::SniMenuItem(item.id.clone()),
                        is_submenu: !item.children.is_empty(),
                    },
                    score,
                    source_index: current_source_index,
                });
            }
        }

        collect_search_sni_entries(&item.children, query, labels, path, source_index, results);
        path.pop();
        labels.pop();
    }
}

fn current_menu_entries(menu: &crate::config::MenuConfig, path: &[usize]) -> Vec<PanelMenuEntry> {
    let items = if path.is_empty() {
        &menu.items
    } else {
        menu_item_at_path(&menu.items, path)
            .map(|item| item.children.as_slice())
            .unwrap_or(&[])
    };
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let mut item_path = path.to_vec();
            item_path.push(index);
            PanelMenuEntry {
                label: item.label.clone(),
                icon: item.icon.clone(),
                target: PanelMenuEntryTarget::StaticPath(item_path.clone()),
                path: item_path,
                is_submenu: !item.children.is_empty(),
            }
        })
        .collect()
}

pub(super) fn search_menu_entries(
    menu: &crate::config::MenuConfig,
    query: &str,
) -> Vec<PanelMenuEntry> {
    if panel_search::is_blank_query(query) {
        return current_menu_entries(menu, &[]);
    }

    let mut results = Vec::new();
    collect_search_menu_entries(
        &menu.items,
        query,
        &mut Vec::new(),
        &mut Vec::new(),
        &mut 0,
        &mut results,
    );
    panel_search::rank_matches(results)
}

fn collect_search_menu_entries(
    items: &[crate::config::MenuItemConfig],
    query: &str,
    labels: &mut Vec<String>,
    path: &mut Vec<usize>,
    source_index: &mut usize,
    results: &mut Vec<SearchMatch<PanelMenuEntry>>,
) {
    for (index, item) in items.iter().enumerate() {
        labels.push(item.label.clone());
        path.push(index);
        let current_source_index = *source_index;
        *source_index += 1;

        let breadcrumb = labels
            .iter()
            .take(labels.len().saturating_sub(1))
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" / ");
        let keyword_text = item.keywords.join(" ");
        let fields = [
            SearchText {
                field: SearchField::Label,
                text: item.label.as_str(),
            },
            SearchText {
                field: SearchField::Secondary,
                text: keyword_text.as_str(),
            },
            SearchText {
                field: SearchField::Breadcrumb,
                text: breadcrumb.as_str(),
            },
        ];

        if let Some(mut score) = panel_search::best_match(&fields, query) {
            if !item.children.is_empty() {
                score.score -= 1;
            }
            results.push(SearchMatch {
                item: PanelMenuEntry {
                    label: labels.join(" / "),
                    icon: item.icon.clone(),
                    path: path.clone(),
                    target: PanelMenuEntryTarget::StaticPath(path.clone()),
                    is_submenu: !item.children.is_empty(),
                },
                score,
                source_index: current_source_index,
            });
        }

        collect_search_menu_entries(&item.children, query, labels, path, source_index, results);
        path.pop();
        labels.pop();
    }
}

fn menu_item_at_path<'a>(
    items: &'a [crate::config::MenuItemConfig],
    path: &[usize],
) -> Option<&'a crate::config::MenuItemConfig> {
    let (first, rest) = path.split_first()?;
    let item = items.get(*first)?;
    if rest.is_empty() {
        Some(item)
    } else {
        menu_item_at_path(&item.children, rest)
    }
}

fn installed_app_entries(
    apps: &[crate::desktop_apps::DesktopApplication],
    query: &str,
) -> Vec<PanelMenuEntry> {
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for app in apps {
        *counts.entry(app.name.to_ascii_lowercase()).or_default() += 1;
    }

    let mut entries = Vec::new();
    for (index, app) in apps.iter().enumerate() {
        let label = if counts
            .get(&app.name.to_ascii_lowercase())
            .copied()
            .unwrap_or(0)
            > 1
        {
            app.generic_name
                .as_ref()
                .filter(|value| !value.is_empty())
                .map(|generic| format!("{} ({generic})", app.name))
                .unwrap_or_else(|| format!("{} ({})", app.name, app.id))
        } else {
            app.name.clone()
        };
        let icon = app
            .icon
            .as_ref()
            .filter(|icon| crate::icon_theme::is_valid_icon_name(icon))
            .cloned();
        let view = PanelMenuEntry {
            label,
            icon,
            path: vec![index],
            target: PanelMenuEntryTarget::InstalledApp(app.id.clone()),
            is_submenu: false,
        };

        if panel_search::is_blank_query(query) {
            entries.push(SearchMatch {
                item: view,
                score: panel_search::best_match(
                    &[SearchText {
                        field: SearchField::Label,
                        text: app.name.as_str(),
                    }],
                    "",
                )
                .expect("blank query matches"),
                source_index: index,
            });
            continue;
        }

        let generic_name = app.generic_name.as_deref().unwrap_or("");
        let comment = app.comment.as_deref().unwrap_or("");
        let keywords = app.keywords.join(" ");
        let fields = [
            SearchText {
                field: SearchField::Label,
                text: app.name.as_str(),
            },
            SearchText {
                field: SearchField::Secondary,
                text: generic_name,
            },
            SearchText {
                field: SearchField::Secondary,
                text: comment,
            },
            SearchText {
                field: SearchField::Secondary,
                text: keywords.as_str(),
            },
        ];
        if let Some(score) = panel_search::best_match(&fields, query) {
            entries.push(SearchMatch {
                item: view,
                score,
                source_index: index,
            });
        }
    }

    panel_search::rank_matches(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dmenu_entries(
        request: crate::ipc::protocol::DmenuRequest,
        query: &str,
    ) -> Vec<PanelMenuEntry> {
        let mut session = PanelMenuSession::open(PanelMenuSource::Dmenu(request));
        session.set_search_query(query.into());
        session.snapshot().entries.to_vec()
    }

    fn installed_app(name: &str, id: &str) -> crate::desktop_apps::DesktopApplication {
        crate::desktop_apps::DesktopApplication {
            id: id.into(),
            name: name.into(),
            generic_name: None,
            comment: None,
            keywords: vec![],
            icon: None,
            exec: name.to_ascii_lowercase(),
        }
    }

    #[test]
    fn dmenu_entries_use_input_as_label_and_value() {
        let entries = dmenu_entries(
            crate::ipc::protocol::DmenuRequest {
                prompt: Some("Run".into()),
                entries: vec![
                    crate::ipc::protocol::DmenuEntry::plain("firefox"),
                    crate::ipc::protocol::DmenuEntry::plain("kitty"),
                    crate::ipc::protocol::DmenuEntry::plain("firefox"),
                ],
                allow_custom: true,
            },
            "",
        );

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["firefox", "kitty", "firefox"]
        );
        assert!(matches!(
            entries[0].target,
            PanelMenuEntryTarget::DmenuValue(_)
        ));
    }

    #[test]
    fn dmenu_custom_value_entry_appears_when_query_has_no_exact_match() {
        let entries = dmenu_entries(
            crate::ipc::protocol::DmenuRequest {
                prompt: None,
                entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
                allow_custom: true,
            },
            "kitty",
        );

        assert_eq!(entries[0].label, "kitty");
        assert!(matches!(
            entries[0].target,
            PanelMenuEntryTarget::DmenuCustomValue(_)
        ));
    }

    #[test]
    fn installed_apps_search_matches_keywords_and_comment() {
        let mut browser = installed_app("Browser", "browser.desktop");
        browser.comment = Some("Browse the web".into());
        browser.keywords = vec!["internet".into()];
        let mut session = PanelMenuSession::open(PanelMenuSource::InstalledApps {
            prompt: Some("Apps".into()),
            show_icons: true,
            apps: vec![browser, installed_app("Editor", "editor.desktop")],
        });

        session.set_search_query("internet".into());
        let snapshot = session.snapshot();

        assert_eq!(snapshot.prompt.as_deref(), Some("Apps"));
        assert!(snapshot.show_icons);
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.entries[0].label, "Browser");
    }

    #[test]
    fn configured_submenu_activation_enters_child_level() {
        let mut session =
            PanelMenuSession::open(PanelMenuSource::Configured(menu_with_apps_browser()));

        assert_eq!(session.activate_index(0), SelectedMenuEntry::None);
        let snapshot = session.snapshot();

        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.entries[0].label, "Browser");
    }

    #[test]
    fn configured_leaf_activation_returns_generic_path() {
        let mut session =
            PanelMenuSession::open(PanelMenuSource::Configured(menu_with_apps_browser()));
        session.activate_index(0);

        let selected = session.activate_focused();
        assert_eq!(selected, SelectedMenuEntry::ConfiguredPath(vec![0, 0]));
        let SelectedMenuEntry::ConfiguredPath(path) = selected else {
            panic!("expected configured path");
        };
        assert_eq!(
            session.configured_action_for_path(&path),
            Some(crate::config::PanelAction::Command(
                crate::config::CommandConfig::Shell("firefox".into())
            ))
        );
    }

    fn menu_with_apps_browser() -> crate::config::MenuConfig {
        crate::config::MenuConfig {
            name: "main".into(),
            show_icons: true,
            prompt: Some("Search".into()),
            source: None,
            items: vec![crate::config::MenuItemConfig {
                label: "Apps".into(),
                icon: None,
                keywords: vec![],
                action: None,
                children: vec![crate::config::MenuItemConfig {
                    label: "Browser".into(),
                    icon: Some("firefox".into()),
                    keywords: vec!["web".into()],
                    action: Some(crate::config::PanelAction::Command(
                        crate::config::CommandConfig::Shell("firefox".into()),
                    )),
                    children: vec![],
                }],
            }],
        }
    }
}
