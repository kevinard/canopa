pub(super) use super::panel_menu_session::{
    PanelMenuEntry as SelectableMenuEntry, PanelMenuEntryTarget as SelectableMenuEntryTarget,
    PanelMenuSession as SelectableMenuSession, PanelMenuSnapshot as SelectableMenuSnapshot,
    PanelMenuSource as SelectableMenuSource, SelectedMenuEntry,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectable_session_returns_generic_selected_entry_for_dmenu() {
        let request = crate::ipc::protocol::DmenuRequest {
            prompt: Some("Run".into()),
            entries: vec![crate::ipc::protocol::DmenuEntry::plain("firefox")],
            allow_custom: true,
        };
        let mut session = SelectableMenuSession::open(SelectableMenuSource::Dmenu(request));

        assert_eq!(
            session.activate_focused(),
            SelectedMenuEntry::DmenuValue("firefox".into())
        );
    }

    #[test]
    fn non_searchable_sni_session_activates_generic_sni_item() {
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![crate::surface::menu_popup::MenuItem {
                id: "42".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: Vec::new(),
                is_separator: false,
            }],
            anchor: (0, 0),
            screen_size: (1920, 1080),
        };
        let mut session = SelectableMenuSession::open(SelectableMenuSource::SniMenu {
            tree,
            search_enabled: false,
        });

        let snapshot = session.snapshot();
        assert!(!snapshot.search_enabled);
        assert_eq!(snapshot.entries[0].label, "Open");
        assert_eq!(
            session.activate_index(0),
            SelectedMenuEntry::SniMenuItem("42".into())
        );
    }

    #[test]
    fn sni_session_entries_skip_non_selectable_items() {
        let item = |id: &str, label: &str, enabled: bool, visible: bool, is_separator: bool| {
            crate::surface::menu_popup::MenuItem {
                id: id.into(),
                label: label.into(),
                enabled,
                visible,
                icon_name: None,
                children: Vec::new(),
                is_separator,
            }
        };
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![
                item("0", "", false, true, true),
                item("1", "Disabled", false, true, false),
                item("2", "Hidden", true, false, false),
                item("3", "Open", true, true, false),
                item("4", "Close", true, true, false),
            ],
            anchor: (0, 0),
            screen_size: (1920, 1080),
        };
        let mut session = SelectableMenuSession::open(SelectableMenuSource::SniMenu {
            tree,
            search_enabled: false,
        });

        let snapshot = session.snapshot();
        assert_eq!(
            snapshot
                .entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Open", "Close"]
        );

        session.move_focus(1);
        assert_eq!(session.snapshot().focused_index, Some(0));
    }

    #[test]
    fn searchable_sni_session_matches_nested_entries_with_breadcrumbs() {
        let child = crate::surface::menu_popup::MenuItem {
            id: "9".into(),
            label: "Suspend".into(),
            enabled: true,
            visible: true,
            icon_name: None,
            children: Vec::new(),
            is_separator: false,
        };
        let tree = crate::surface::menu_popup::MenuTree {
            items: vec![crate::surface::menu_popup::MenuItem {
                id: "8".into(),
                label: "Power".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![child],
                is_separator: false,
            }],
            anchor: (0, 0),
            screen_size: (1920, 1080),
        };
        let mut session = SelectableMenuSession::open(SelectableMenuSource::SniMenu {
            tree,
            search_enabled: true,
        });

        session.set_search_query("power suspend".into());
        let snapshot = session.snapshot();

        assert!(snapshot.search_enabled);
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.entries[0].label, "Power / Suspend");
        assert_eq!(
            session.activate_index(0),
            SelectedMenuEntry::SniMenuItem("9".into())
        );
    }
}
