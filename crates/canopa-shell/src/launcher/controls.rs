//! Panels and controls. Settings pages come from the Settings registry
//! through [`super::system`].
use super::{Action, NamedAction, ResultItem};
use crate::app::panel_search::{SearchField, SearchMatch, SearchText, best_match, rank_matches};

pub(crate) fn results(query: &str, scoped: bool) -> Vec<ResultItem> {
    if query.is_empty() && !scoped {
        return vec![];
    }
    // English keywords always match; the session language adds its own.
    let keywords = |english: &str, translated: String| format!("{english} {translated}");
    let controls = [
        (
            "wifi",
            fl!("shell-control-wifi"),
            keywords(
                "wifi wi-fi wireless internet network connect",
                fl!("shell-control-wifi-keywords"),
            ),
            "network-wireless",
            Action::Provider {
                name: "wifi".into(),
            },
        ),
        (
            "sound",
            fl!("shell-control-sound"),
            keywords(
                "sound audio volume speaker microphone output input",
                fl!("shell-control-sound-keywords"),
            ),
            "audio-volume-high",
            Action::Provider {
                name: "audio".into(),
            },
        ),
        (
            "notifications",
            fl!("shell-control-notifications"),
            keywords(
                "notifications do not disturb dnd alerts center",
                fl!("shell-control-notifications-keywords"),
            ),
            "preferences-system-notifications",
            Action::Provider {
                name: "notifications".into(),
            },
        ),
        (
            "settings",
            fl!("shell-control-settings"),
            keywords(
                "settings preferences desktop configuration",
                fl!("shell-control-settings-keywords"),
            ),
            "preferences-system",
            Action::Launch {
                argv: vec!["canopa-settings".into()],
            },
        ),
        (
            "history",
            fl!("shell-control-clear-history"),
            keywords(
                "clear forget reset launcher history recent frequent ranking",
                fl!("shell-control-clear-history-keywords"),
            ),
            "edit-clear",
            Action::ClearHistory,
        ),
        (
            "bluetooth",
            fl!("shell-control-bluetooth"),
            keywords(
                "bluetooth headphones discover pair reconnect",
                fl!("shell-control-bluetooth-keywords"),
            ),
            "bluetooth",
            Action::Provider {
                name: "bluetooth".into(),
            },
        ),
        (
            "power-brightness",
            fl!("shell-control-power"),
            keywords(
                "brightness backlight performance balanced power saver profile",
                fl!("shell-control-power-keywords"),
            ),
            "display-brightness",
            Action::Provider {
                name: "power".into(),
            },
        ),
    ];
    let matches = controls
        .into_iter()
        .enumerate()
        .filter_map(|(source_index, (id, label, keywords, icon, action))| {
            let score = best_match(
                &[
                    SearchText {
                        field: SearchField::Label,
                        text: &label,
                    },
                    SearchText {
                        field: SearchField::Label,
                        text: &keywords,
                    },
                ],
                query,
            )?;
            Some(SearchMatch {
                source_index,
                score,
                item: ResultItem {
                    id: format!("control:{id}"),
                    label,
                    description: fl!("shell-control-description"),
                    icon: Some(icon.into()),
                    actions: vec![NamedAction {
                        label: fl!("shell-launcher-open"),
                        action,
                    }],
                },
            })
        })
        .collect();
    rank_matches(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controls_use_existing_domain_interfaces() {
        assert!(
            matches!(&results("volume",false)[0].actions[0].action,Action::Provider{name} if name=="audio")
        );
        assert!(
            matches!(&results("dnd",false)[0].actions[0].action,Action::Provider{name} if name=="notifications")
        );
        assert_eq!(results("", true).len(), 7);
        assert!(results("", false).is_empty());
        assert!(results("foobarxyz", false).is_empty());
    }
}
