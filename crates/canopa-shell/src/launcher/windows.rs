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
                        format!(
                            "Workspace {}{}",
                            workspace
                                .name
                                .clone()
                                .unwrap_or_else(|| workspace.idx.to_string()),
                            workspace
                                .output
                                .as_ref()
                                .map(|output| format!(" · {output}"))
                                .unwrap_or_default()
                        )
                    })
                    .unwrap_or_else(|| "Workspace unavailable".into()),
            }
        })
        .collect();
    windows.sort_by_key(|window| window.id);
    windows
}

fn application<'a>(id: &str, apps: &'a [DesktopApplication]) -> Option<&'a DesktopApplication> {
    apps.iter().find(|app| {
        app.id
            .trim_end_matches(".desktop")
            .eq_ignore_ascii_case(id.trim_end_matches(".desktop"))
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
        let name = app
            .map(|app| app.name.as_str())
            .or(window.application.as_deref())
            .unwrap_or("Application");
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
                label: "Switch to window".into(),
                action: Action::FocusWindow {
                    id: window.id,
                    application: app
                        .map(|app| app.id.clone())
                        .or_else(|| window.application.clone()),
                },
            }];
            if let Some(app) = app {
                actions.push(NamedAction {
                    label: "Launch application".into(),
                    action: Action::Application { id: app.id.clone() },
                });
            }
            matches.push(SearchMatch {
                item: ResultItem {
                    id: format!("window:{}", window.id),
                    label: label.into(),
                    description: format!("Window · {name} · {}", window.workspace),
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
}
