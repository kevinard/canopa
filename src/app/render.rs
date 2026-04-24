use iced::alignment::{Horizontal, Vertical};

use super::App;
use crate::config::Edge;
use crate::surface::menu_popup::{MenuTree, popup_size};

pub(super) struct PopupRenderModel<'a> {
    pub(super) service: &'a str,
    pub(super) menu_path: &'a zbus::zvariant::OwnedObjectPath,
    pub(super) tree: &'a MenuTree,
    pub(super) placement: PopupPlacementModel,
}

pub(super) struct PopupPlacementModel {
    pub(super) h_align: Horizontal,
    pub(super) v_align: Vertical,
    pub(super) padding: iced::Padding,
    pub(super) popup_width: u32,
    pub(super) popup_height: u32,
}

impl App {
    pub(super) fn popup_render_model(
        &self,
        window: iced::window::Id,
    ) -> Option<PopupRenderModel<'_>> {
        let meta = self.surfaces.popup(window)?;
        let bar = Self::bar_for_surface_name(&self.config.bars, &meta.bar_name);

        Some(PopupRenderModel {
            service: meta.service.as_str(),
            menu_path: &meta.menu_path,
            tree: &meta.tree,
            placement: popup_placement_model(&bar.edge, bar.collapsed_width as f32, &meta.tree),
        })
    }
}

fn popup_placement_model(edge: &Edge, bar_size: f32, tree: &MenuTree) -> PopupPlacementModel {
    let (popup_width, popup_height) = popup_size(tree);
    let screen_w = tree.screen_size.0 as f32;
    let screen_h = tree.screen_size.1 as f32;
    let raw_cursor_x = tree.anchor.0 as f32;
    let raw_cursor_y = tree.anchor.1 as f32;
    let cursor_x = if screen_w > 0.0 {
        raw_cursor_x.min(screen_w - popup_width as f32).max(0.0)
    } else {
        raw_cursor_x
    };
    let cursor_y = if screen_h > 0.0 {
        raw_cursor_y.min(screen_h - popup_height as f32).max(0.0)
    } else {
        raw_cursor_y
    };

    let (h_align, v_align, padding) = match edge {
        Edge::Left => (
            Horizontal::Left,
            Vertical::Top,
            iced::Padding {
                top: cursor_y,
                left: bar_size,
                ..Default::default()
            },
        ),
        Edge::Right => (
            Horizontal::Right,
            Vertical::Top,
            iced::Padding {
                top: cursor_y,
                right: bar_size,
                ..Default::default()
            },
        ),
        Edge::Top => (
            Horizontal::Left,
            Vertical::Top,
            iced::Padding {
                top: bar_size,
                left: cursor_x,
                ..Default::default()
            },
        ),
        Edge::Bottom => (
            Horizontal::Left,
            Vertical::Bottom,
            iced::Padding {
                bottom: bar_size,
                left: cursor_x,
                ..Default::default()
            },
        ),
    };

    PopupPlacementModel {
        h_align,
        v_align,
        padding,
        popup_width,
        popup_height,
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Edge;
    use crate::surface::menu_popup::{MenuItem, MenuTree};

    fn tree(anchor: (i32, i32), screen_size: (u32, u32)) -> MenuTree {
        MenuTree {
            items: vec![MenuItem {
                id: "1".into(),
                label: "Open".into(),
                enabled: true,
                visible: true,
                icon_name: None,
                children: vec![],
                is_separator: false,
            }],
            anchor,
            screen_size,
        }
    }

    #[test]
    fn popup_placement_uses_edge_relative_alignment_and_bar_padding() {
        let tree = tree((40, 30), (500, 400));

        let left = super::popup_placement_model(&Edge::Left, 48.0, &tree);
        assert_eq!(left.padding.left, 48.0);
        assert_eq!(left.padding.top, 30.0);

        let right = super::popup_placement_model(&Edge::Right, 48.0, &tree);
        assert_eq!(right.padding.right, 48.0);
        assert_eq!(right.padding.top, 30.0);

        let top = super::popup_placement_model(&Edge::Top, 48.0, &tree);
        assert_eq!(top.padding.top, 48.0);
        assert_eq!(top.padding.left, 40.0);

        let bottom = super::popup_placement_model(&Edge::Bottom, 48.0, &tree);
        assert_eq!(bottom.padding.bottom, 48.0);
        assert_eq!(bottom.padding.left, 40.0);
    }

    #[test]
    fn popup_placement_clamps_anchor_to_screen_bounds() {
        let tree = tree((490, 390), (500, 400));
        let placement = super::popup_placement_model(&Edge::Top, 48.0, &tree);

        assert_eq!(placement.padding.left, 350.0);
        assert_eq!(placement.padding.top, 48.0);
    }

    #[test]
    fn popup_placement_preserves_anchor_when_screen_size_is_unknown() {
        let tree = tree((490, 390), (0, 0));
        let placement = super::popup_placement_model(&Edge::Top, 48.0, &tree);

        assert_eq!(placement.padding.left, 490.0);
        assert_eq!(placement.padding.top, 48.0);
    }
}
