/// A generic menu tree, passed from shared app state to a popup SurfaceState.
/// No knowledge of SNI, DBusMenu, or any module internals.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuTree {
    pub items: Vec<MenuItem>,
    /// Screen position (x, y) adjacent to which the popup should appear.
    pub anchor: (i32, i32),
    /// Logical screen size (width, height) of the output the menu appears on.
    /// Used to clamp the popup so it does not overflow screen edges.
    pub screen_size: (u32, u32),
}

/// One item in a `MenuTree`. Children are ignored for MVP (no nested submenus).
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    /// Opaque string ID; the caller interprets it on selection.
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub visible: bool,
    /// MVP does not render icons; retained for future submenu support.
    pub icon_name: Option<String>,
    /// Submenus -- not rendered in MVP; retained for future nesting support.
    pub children: Vec<MenuItem>,
    pub is_separator: bool,
}

/// Render a popup menu from a `MenuTree`.
///
/// Returns an Iced `Element` that renders a vertical list of menu items.
/// Separators become thin horizontal rules. Disabled items are dimmed.
/// The hovered enabled item is highlighted with a `SURFACE0` background.
/// The host's Wayland popup grab handles outside-click dismissal.
pub fn view_menu_popup<'a>(
    tree: &'a MenuTree,
    service: &'a str,
    menu_path: &'a zbus::zvariant::OwnedObjectPath,
    window: iced::window::Id,
    hovered_item: Option<&str>,
    focused_item: Option<&str>,
) -> iced::Element<'a, crate::app::Message> {
    use iced::Length;
    use iced::widget::{column, container, rule};

    let ui = crate::theme::ui(canopa_theme::UiKind::Menu);
    let (row_ui, row_height) = popup_row_metrics();
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
                let label = crate::theme::text(&item.label)
                    .size(crate::theme::body_size() * 13.0 / 14.0)
                    .wrapping(iced::widget::text::Wrapping::None);
                let hovered = item.enabled && hovered_item == Some(item.id.as_str());
                let focused = item.enabled && focused_item == Some(item.id.as_str());
                let row = container(label)
                    .padding(canopa_ui::content_padding(row_ui))
                    .height(row_height)
                    .width(Length::Fill)
                    .style(move |_| {
                        crate::theme::highlighted_frame(
                            row_ui,
                            focused,
                            hovered,
                            !item.enabled,
                            focused,
                        )
                    });
                let row = container(row).padding(canopa_ui::padding(row_ui.margin));

                if item.enabled {
                    let menu_item_id: i32 = item.id.parse().unwrap_or(0);
                    let menu_path = menu_path.clone();
                    let service = service.to_string();
                    let item_id_enter = item.id.clone();
                    // on_exit is NOT on individual items -- it belongs on the container below.
                    // Per-item on_exit fires when moving to an adjacent item, momentarily clearing
                    // the hover state before the next on_enter fires, causing inconsistent highlights.
                    iced::widget::mouse_area(row)
                        .on_press(crate::app::Message::Menu(
                            crate::app::messages::MenuEvent::ItemSelected {
                                service,
                                menu_item_id,
                                menu_path,
                            },
                        ))
                        .on_enter(crate::app::Message::Menu(
                            crate::app::messages::MenuEvent::ItemHoverEnter {
                                window,
                                item_id: item_id_enter,
                            },
                        ))
                        .into()
                } else {
                    row.into()
                }
            }
        })
        .collect();

    // Wrap the entire item list in a mouse_area so hover clears only when the pointer
    // leaves the menu, not when crossing between adjacent items.
    let menu_content = iced::widget::mouse_area(
        container(column(items).spacing(ui.gap.px()))
            .style(move |_| canopa_ui::frame(&crate::theme::appearance(), ui))
            .padding(canopa_ui::content_padding(ui)),
    )
    .on_exit(crate::app::Message::Menu(
        crate::app::messages::MenuEvent::ItemHoverExit(window),
    ));

    container(menu_content)
        .padding(canopa_ui::padding(ui.margin))
        .into()
}

/// Estimate the pixel size `(width, height)` needed to display `tree`.
///
/// Used to size the layer-shell popup window before it is opened.
/// All measurements are in logical pixels.
///
/// Per-item heights (empirically tuned for Iced default font at size 13):
///   - enabled / disabled item: padding [4, 12] (8px) + ~17px line height (size 13 * LineHeight::Relative(1.3)) + 1px rounding = 26px
///   - separator: padding [2, 8] + 1px rule = 5px
///   - outer container: padding(4) = 8px vertical, 8px horizontal
///
/// Width = max(MIN_WIDTH, longest_label_chars * CHAR_WIDTH + item_h_padding + outer_h_padding)
pub(crate) fn popup_row_metrics() -> (canopa_theme::ResolvedStyle, f32) {
    let appearance = crate::theme::appearance();
    let mut base = canopa_theme::Appearance::default().style(canopa_theme::UiKind::MenuRow);
    base.padding = canopa_theme::Insets::axes(4, 12);
    base.radius = [canopa_theme::Pixels::whole(3); 4];
    base.border_color = canopa_theme::ColorValue::Rgba(canopa_theme::Rgb(0, 0, 0), 0);
    let ui = base.apply(appearance.customization.ui[canopa_theme::UiKind::MenuRow as usize]);
    let border = if ui.custom_border {
        2.0 * ui.border_width.px()
    } else {
        0.0
    };
    let height = (crate::theme::body_size() * 13.0 / 14.0 * 1.3 + ui.padding.vertical() + border)
        .ceil()
        + 1.0;
    (ui, height)
}

pub fn popup_size(tree: &MenuTree) -> (u32, u32) {
    let ui = crate::theme::ui(canopa_theme::UiKind::Menu);
    let (row, row_height) = popup_row_metrics();
    let visible = tree.items.iter().filter(|i| i.visible).collect::<Vec<_>>();
    let height = visible
        .iter()
        .map(|i| {
            if i.is_separator {
                5.0
            } else {
                row_height + row.margin.vertical()
            }
        })
        .sum::<f32>()
        + ui.padding.vertical()
        + if ui.custom_border {
            2.0 * ui.border_width.px()
        } else {
            0.0
        }
        + ui.margin.vertical()
        + ui.gap.px() * visible.len().saturating_sub(1) as f32;
    let measured = visible
        .iter()
        .filter(|i| !i.is_separator)
        .map(|i| {
            crate::theme::text_width(&i.label, crate::theme::body_size() * 13.0 / 14.0)
                .max(i.label.chars().count() as f32 * 8.0 * crate::theme::body_size() / 14.0)
        })
        .fold(0.0, f32::max);
    let width = (measured
        + row.padding.horizontal()
        + if row.custom_border {
            2.0 * row.border_width.px()
        } else {
            0.0
        }
        + row.margin.horizontal()
        + ui.padding.horizontal()
        + if ui.custom_border {
            2.0 * ui.border_width.px()
        } else {
            0.0
        }
        + ui.margin.horizontal())
    .max(150.0);
    (width.ceil() as u32, height.ceil() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_size_single_item() {
        let tree = MenuTree {
            items: vec![MenuItem {
                id: "1".into(),
                label: "Open".into(), // 4 chars
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            }],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let (w, h) = popup_size(&tree);
        // height: 1 item * 26 + 16 outer = 42
        assert_eq!(h, 42);
        // width: max(150, 4*8 + 24 + 16) = max(150, 72) = 150
        assert_eq!(w, 150);
    }

    #[test]
    fn popup_size_separator_uses_sep_height() {
        let tree = MenuTree {
            items: vec![
                MenuItem {
                    id: "1".into(),
                    label: "Item".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
                MenuItem {
                    id: "0".into(),
                    label: String::new(),
                    enabled: false,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: true,
                },
                MenuItem {
                    id: "2".into(),
                    label: "Item".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
            ],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let (_, h) = popup_size(&tree);
        // 2 items * 26 + 1 sep * 5 + 16 outer + 2 gaps * 4 = 81
        assert_eq!(h, 81);
    }

    #[test]
    fn popup_size_hidden_items_excluded() {
        let tree = MenuTree {
            items: vec![
                MenuItem {
                    id: "1".into(),
                    label: "Visible".into(),
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
                MenuItem {
                    id: "2".into(),
                    label: "Hidden".into(),
                    enabled: true,
                    visible: false,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
            ],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let (_, h) = popup_size(&tree);
        // Only 1 visible item: 26 + 16 = 42
        assert_eq!(h, 42);
    }

    #[test]
    fn popup_size_width_driven_by_longest_label() {
        let tree = MenuTree {
            items: vec![
                MenuItem {
                    id: "1".into(),
                    label: "Hi".into(),
                    enabled: true,
                    visible: true, // 2 chars
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
                MenuItem {
                    id: "2".into(),
                    label: "A very long label here!".into(), // 23 chars
                    enabled: true,
                    visible: true,
                    icon_name: None,
                    children: vec![],
                    is_separator: false,
                },
            ],
            anchor: (0, 0),
            screen_size: (0, 0),
        };
        let (w, _) = popup_size(&tree);
        // width: max(150, 23*8 + 24 + 16) = max(150, 224) = 224
        assert_eq!(w, 224);
    }

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
            screen_size: (0, 0),
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
