/// A generic menu tree, passed from SharedState to a popup SurfaceState.
/// No knowledge of SNI, DBusMenu, or any module internals.
#[derive(Debug, Clone)]
pub struct MenuTree {
    pub items: Vec<MenuItem>,
    /// Screen position (x, y) adjacent to which the popup should appear.
    pub anchor: (i32, i32),
}

/// One item in a `MenuTree`. Children are ignored for MVP (no nested submenus).
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Opaque string ID; the caller interprets it on selection.
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub visible: bool,
    pub icon_name: Option<String>,
    pub children: Vec<MenuItem>,
    pub is_separator: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_tree_stores_items() {
        let tree = MenuTree {
            items: vec![MenuItem {
                id: "1".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            }],
            anchor: (100, 200),
        };
        assert_eq!(tree.items.len(), 1);
        assert_eq!(tree.anchor, (100, 200));
    }

    #[test]
    fn menu_item_separator_has_no_label() {
        let sep = MenuItem {
            id: "0".into(),
            label: String::new(),
            enabled: false,
            visible: true,
            icon_name: None,
            children: vec![],
            is_separator: true,
        };
        assert!(sep.is_separator);
        assert!(sep.label.is_empty());
    }

    #[test]
    fn menu_item_disabled_is_tracked() {
        let item = MenuItem {
            id: "5".into(),
            label: "Grayed Out".into(),
            enabled: false,
            visible: true,
            icon_name: None,
            children: vec![],
            is_separator: false,
        };
        assert!(!item.enabled);
    }
}
