use super::{Action, NamedAction, ResultItem};
use crate::app::panel_search::{SearchField, SearchMatch, SearchText, best_match, rank_matches};

pub(crate) fn results(query: &str, scoped: bool) -> Vec<ResultItem> {
    if query.is_empty() && !scoped {
        return vec![];
    }
    let controls = [
        (
            "wifi",
            "Wi-Fi networks",
            "wifi wi-fi wireless internet network connect",
            "network-wireless",
            Action::Provider {
                name: "wifi".into(),
            },
        ),
        (
            "sound",
            "Sound and volume",
            "sound audio volume speaker microphone output input",
            "audio-volume-high",
            Action::Provider {
                name: "audio".into(),
            },
        ),
        (
            "notifications",
            "Notifications and Do Not Disturb",
            "notifications do not disturb dnd alerts center",
            "preferences-system-notifications",
            Action::Provider {
                name: "notifications".into(),
            },
        ),
        (
            "settings",
            "Canopa Settings",
            "settings preferences desktop configuration",
            "preferences-system",
            Action::Launch {
                argv: vec!["canopa-settings".into()],
            },
        ),
        (
            "displays",
            "Display settings",
            "displays monitor screen resolution scale rotation",
            "video-display",
            Action::Launch {
                argv: vec!["canopa-settings".into(), "--page".into(), "displays".into()],
            },
        ),
        (
            "input",
            "Keyboard and pointer settings",
            "keyboard mouse pointer touchpad input layout",
            "input-keyboard",
            Action::Launch {
                argv: vec!["canopa-settings".into(), "--page".into(), "input".into()],
            },
        ),
        (
            "appearance",
            "Personalization settings",
            "personalization theme appearance dark light wallpaper accent text size accessibility motion",
            "preferences-desktop-theme",
            Action::Launch {
                argv: vec!["canopa-settings".into(), "--page".into(), "personal".into()],
            },
        ),
        (
            "power",
            "Power and idle settings",
            "power idle suspend lock sleep battery",
            "preferences-system-power",
            Action::Launch {
                argv: vec!["canopa-settings".into(), "--page".into(), "power".into()],
            },
        ),
        (
            "launcher",
            "Launcher settings",
            "launcher extensions plugins web search keywords aliases pins clipboard snippets images",
            "preferences-system",
            Action::Launch {
                argv: vec!["canopa-settings".into(), "--page".into(), "launcher".into()],
            },
        ),
        (
            "history",
            "Clear launcher history",
            "clear forget reset launcher history recent frequent ranking",
            "edit-clear",
            Action::ClearHistory,
        ),
        (
            "bluetooth",
            "Bluetooth devices",
            "bluetooth headphones discover pair reconnect",
            "bluetooth",
            Action::Provider {
                name: "bluetooth".into(),
            },
        ),
        (
            "power-brightness",
            "Power & brightness",
            "brightness backlight performance balanced power saver profile",
            "display-brightness",
            Action::Provider {
                name: "power".into(),
            },
        ),
        (
            "shortcuts",
            "Keyboard shortcuts",
            "shortcuts keybindings hotkeys rebind",
            "input-keyboard",
            Action::Launch {
                argv: vec![
                    "canopa-settings".into(),
                    "--page".into(),
                    "shortcuts".into(),
                ],
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
                        text: label,
                    },
                    SearchText {
                        field: SearchField::Label,
                        text: keywords,
                    },
                ],
                query,
            )?;
            Some(SearchMatch {
                source_index,
                score,
                item: ResultItem {
                    id: format!("control:{id}"),
                    label: label.into(),
                    description: "Settings and controls".into(),
                    icon: Some(icon.into()),
                    actions: vec![NamedAction {
                        label: "Open".into(),
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
        assert_eq!(results("", true).len(), 13);
        assert!(results("", false).is_empty());
        assert!(results("foobarxyz", false).is_empty());
    }
}
