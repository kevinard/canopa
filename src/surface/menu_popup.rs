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
    /// Submenus -- not rendered in MVP.
    pub children: Vec<MenuItem>,
    pub is_separator: bool,
}

/// Render a popup menu from a `MenuTree`.
///
/// Returns an Iced `Element` that renders a vertical list of menu items.
/// Separators become thin horizontal rules. Disabled items are dimmed.
/// The caller must wrap this in a mouse_area for outside-click dismissal.
pub fn view_menu_popup<'a>(
    tree: &'a MenuTree,
    service: &'a str,
    menu_path: &'a zbus::zvariant::OwnedObjectPath,
) -> iced::Element<'a, crate::app::Message> {
    use iced::widget::{column, container, rule, text};
    use iced::Length;

    let items: Vec<iced::Element<'_, crate::app::Message>> = tree
        .items
        .iter()
        .filter(|i| i.visible)
        .map(|item| {
            if item.is_separator {
                container(rule::horizontal(1))
                    .padding([2, 8])
                    .width(Length::Fill)
                    .into()
            } else {
                let color = if item.enabled {
                    crate::theme::TEXT
                } else {
                    crate::theme::OVERLAY0
                };
                let label = text(&item.label).size(13).color(color);
                let row = container(label).padding([4, 12]).width(Length::Fill);

                if item.enabled {
                    let menu_item_id: i32 = item.id.parse().unwrap_or(0);
                    let menu_path = menu_path.clone();
                    let service = service.to_string();
                    iced::widget::mouse_area(row)
                        .on_press(crate::app::Message::MenuItemSelected {
                            service,
                            menu_item_id,
                            menu_path,
                        })
                        .into()
                } else {
                    row.into()
                }
            }
        })
        .collect();

    container(column(items).spacing(0))
        .style(|_| iced::widget::container::Style {
            background: Some(iced::Background::Color(crate::theme::SURFACE1)),
            border: iced::Border {
                color: crate::theme::BORDER,
                width: 1.0,
                radius: iced::border::Radius::from(4.0),
            },
            ..Default::default()
        })
        .padding(4)
        .into()
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
