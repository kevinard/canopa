use super::{Action, NamedAction, ResultItem};
use crate::{
    app::panel_search::{SearchField, SearchMatch, SearchText, best_match, rank_matches},
    desktop_apps::DesktopApplication,
    niri::NiriState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub id: u64,
    pub title: String,
    pub application: Option<String>,
    pub workspace: String,
}

pub(crate) fn snapshot(state: &NiriState) -> Vec<Window> {
    let mut windows: Vec<_> = state
        .windows
        .values()
        .map(|window| {
            let workspace = window
                .workspace_id
                .and_then(|id| state.workspaces.iter().find(|workspace| workspace.id == id));
            Window {
                id: window.id,
                title: window.title.clone().unwrap_or_default(),
                application: window.app_id.clone(),
                workspace: workspace
                    .map(|workspace| {
                        let name = workspace
                            .name
                            .clone()
                            .unwrap_or_else(|| workspace.idx.to_string());
                        match &workspace.output {
                            Some(output) => fl!(
                                "shell-window-workspace-on-output",
                                workspace = name,
                                output = output.clone()
                            ),
                            None => fl!("shell-window-workspace", workspace = name),
                        }
                    })
                    .unwrap_or_else(|| fl!("shell-window-workspace-unavailable")),
            }
        })
        .collect();
    windows.sort_by_key(|window| window.id);
    windows
}

/// Whether a window's app id names `app`: its desktop id, or its
/// `StartupWMClass`, ignoring case.
pub(crate) fn belongs_to(app_id: &str, app: &DesktopApplication) -> bool {
    let app_id = app_id.trim_end_matches(".desktop");
    app.id
        .trim_end_matches(".desktop")
        .eq_ignore_ascii_case(app_id)
        || app
            .startup_wm_class
            .as_deref()
            .is_some_and(|class| class.eq_ignore_ascii_case(app_id))
}

fn application<'a>(id: &str, apps: &'a [DesktopApplication]) -> Option<&'a DesktopApplication> {
    // An exact desktop id wins over another entry's StartupWMClass.
    apps.iter()
        .find(|app| {
            app.id
                .trim_end_matches(".desktop")
                .eq_ignore_ascii_case(id.trim_end_matches(".desktop"))
        })
        .or_else(|| apps.iter().find(|app| belongs_to(id, app)))
}

/// The window "Switch to" focuses for `app`: the oldest of its windows, which
/// is usually its main window. Windows of another entry with that exact id
/// are not counted.
pub(crate) fn window_for<'a>(
    windows: &'a [Window],
    apps: &[DesktopApplication],
    app: &DesktopApplication,
) -> Option<&'a Window> {
    windows.iter().find(|window| {
        window
            .application
            .as_deref()
            .and_then(|id| application(id, apps))
            .is_some_and(|owner| owner.id == app.id)
    })
}

pub(crate) fn results(
    windows: &[Window],
    apps: &[DesktopApplication],
    query: &str,
) -> Vec<ResultItem> {
    let mut matches = Vec::new();
    for (source_index, window) in windows.iter().enumerate() {
        let app = window
            .application
            .as_deref()
            .and_then(|id| application(id, apps));
        let fallback = fl!("shell-launcher-application");
        let name = app
            .map(|app| app.name.as_str())
            .or(window.application.as_deref())
            .unwrap_or(&fallback);
        let label = if window.title.is_empty() {
            name
        } else {
            &window.title
        };
        let fields = [
            SearchText {
                field: SearchField::Label,
                text: label,
            },
            SearchText {
                field: SearchField::Label,
                text: name,
            },
            SearchText {
                field: SearchField::Secondary,
                text: &window.workspace,
            },
        ];
        if let Some(score) = best_match(&fields, query) {
            let mut actions = vec![NamedAction {
                label: fl!("shell-window-switch"),
                action: Action::FocusWindow {
                    id: window.id,
                    application: app
                        .map(|app| app.id.clone())
                        .or_else(|| window.application.clone()),
                },
            }];
            if let Some(app) = app {
                actions.push(NamedAction {
                    label: fl!("shell-window-launch-application"),
                    action: Action::Application { id: app.id.clone() },
                });
            }
            matches.push(SearchMatch {
                item: ResultItem {
                    id: format!("window:{}", window.id),
                    label: label.into(),
                    description: fl!(
                        "shell-window-description",
                        application = name.to_owned(),
                        workspace = window.workspace.clone()
                    ),
                    icon: app.and_then(|app| app.icon.clone()),
                    actions,
                },
                score,
                source_index,
            });
        }
    }
    rank_matches(matches)
        .into_iter()
        .take(super::RESULT_LIMIT)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_match_titles_applications_and_workspace_and_keep_focus_identity() {
        let windows = vec![Window {
            id: 42,
            title: "Project notes".into(),
            application: Some("editor".into()),
            workspace: "Workspace Work · DP-1".into(),
        }];
        for query in ["notes", "editor", "Work", ""] {
            let rows = results(&windows, &[], query);
            assert_eq!(rows.len(), 1);
            assert!(matches!(
                rows[0].actions[0].action,
                Action::FocusWindow { id: 42, .. }
            ));
        }
        assert!(results(&[], &[], "editor").is_empty());
        assert!(results(&windows, &[], "missing").is_empty());
    }

    #[test]
    fn windows_belong_to_apps_by_desktop_id_or_startup_wm_class() {
        let app = |id: &str, class: Option<&str>| DesktopApplication {
            id: id.into(),
            name: id.into(),
            startup_wm_class: class.map(Into::into),
            ..Default::default()
        };
        let apps = vec![
            app("org.example.Editor.desktop", None),
            app("chromium.desktop", Some("Chromium-browser")),
            app("chromium-browser.desktop", None),
        ];
        let window = |id: u64, app_id: &str| Window {
            id,
            title: String::new(),
            application: Some(app_id.into()),
            workspace: String::new(),
        };
        let windows = vec![
            window(3, "chromium-browser"),
            window(5, "org.example.editor"),
            window(7, "org.example.Editor"),
        ];
        assert_eq!(window_for(&windows, &apps, &apps[0]).map(|w| w.id), Some(5));
        // The exact desktop id claims the window before another StartupWMClass.
        assert_eq!(window_for(&windows, &apps, &apps[1]).map(|w| w.id), None);
        assert_eq!(window_for(&windows, &apps, &apps[2]).map(|w| w.id), Some(3));
        let only = vec![apps[1].clone()];
        assert_eq!(window_for(&windows, &only, &only[0]).map(|w| w.id), Some(3));
        assert_eq!(results(&windows[..1], &only, "")[0].icon, None);
        assert!(matches!(
            &results(&windows[..1], &only, "")[0].actions[1].action,
            Action::Application { id } if id == "chromium.desktop"
        ));
    }
}
