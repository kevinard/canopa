use crate::config::{BarConfig, Edge};
use crate::modules::ModuleId;
use crate::surface::panel::{PanelContent, PanelPreferredSize, PanelTree};
use crate::surface::placement::{Placement, Point, Size};

pub(super) const MENU_ROW_EXTENT: f32 = 45.0;

/// Shared by row rendering and background icon preparation.
pub(super) fn menu_render_range(
    count: usize,
    scroll_y: f32,
    viewport_height: f32,
    row_extent: f32,
) -> std::ops::Range<usize> {
    const BUFFER: usize = 8;
    let first_visible = (scroll_y / row_extent).floor().max(0.0) as usize;
    let start = first_visible.saturating_sub(BUFFER).min(count);
    let rows = 40.max((viewport_height / row_extent).ceil() as usize + 2 * BUFFER);
    start..(start + rows).min(count)
}

const DEFAULT_PANEL_FALLBACK_WIDTH: u32 = 360;
const DEFAULT_PANEL_FALLBACK_HEIGHT: u32 = 600;
const NOTIFICATION_CENTER_WIDTH: u32 = 480;
const PANEL_OUTER_PADDING: u32 = 2 * canopa_ui::panel::PADDING as u32;
const PANEL_MODULE_FRAME_H_PADDING: u32 = 20;
const PANEL_MODULE_FRAME_V_PADDING: u32 = 16;
const PANEL_ENTRY_SPACING: u32 = 8;
const PANEL_MENU_MIN_WIDTH: u32 = 240;
const PANEL_MENU_SEARCH_HEIGHT: u32 = 38;
const PANEL_MENU_LIST_PADDING: u32 = 2 * canopa_ui::panel::PADDING as u32;
const PANEL_MENU_ROW_H_PADDING: u32 = 20;
const PANEL_MENU_ICON_SLOT: u32 = 26;
const PANEL_MENU_SUBMENU_SLOT: u32 = 16;

pub(super) struct PanelSizingInput<'a, F>
where
    F: FnMut(&ModuleId) -> Option<PanelPreferredSize>,
{
    pub(super) bar: &'a BarConfig,
    pub(super) ui: canopa_theme::ResolvedStyle,
    pub(super) menu_row_extent: f32,
    pub(super) tree: &'a PanelTree,
    pub(super) surface_origin: Point,
    pub(super) panel_module_layout: &'a [crate::surface::panel::PanelModuleEntry],
    pub(super) menu: super::foreground::PanelMenuRenderState,
    pub(super) module_detail_preferred_size: F,
}

#[derive(Debug, Clone)]
pub(super) struct PanelPhysicalPlan {
    pub(super) edge: Edge,
    pub(super) width: PlannedDimension,
    pub(super) height: PlannedDimension,
    pub(super) placement: PanelPlacement,
    pub(super) output_name: Option<String>,
    /// Offset from the compositor's usable edge, including reserved bar space.
    pub(super) edge_offset: i32,
}

impl PanelPhysicalPlan {
    pub(super) fn layer_size(&self) -> (u32, u32) {
        (self.width.layer_size(), self.height.layer_size())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlannedDimension {
    Exact(u32),
    Fill { span: u32 },
}

impl PlannedDimension {
    pub(super) fn value(self) -> u32 {
        match self {
            Self::Exact(value) | Self::Fill { span: value } => value,
        }
    }

    pub(super) fn fills(self) -> bool {
        matches!(self, Self::Fill { .. })
    }

    fn layer_size(self) -> u32 {
        if self.fills() { 0 } else { self.value() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PanelPlacement {
    pub(super) position: Point,
    pub(super) size: Size,
    pub(super) screen: Size,
}

impl PanelPlacement {
    pub(super) fn layer_margin(&self, edge: &Edge) -> (i32, i32, i32, i32) {
        Placement {
            position: self.position,
            size: self.size,
            screen: self.screen,
            edge,
        }
        .layer_margin()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PanelContentSizeHint {
    preferred_width: Option<u32>,
    preferred_height: Option<u32>,
    fill_edge_span: bool,
    fill_height: bool,
}

pub(super) fn panel_physical_plan<F>(mut input: PanelSizingInput<'_, F>) -> PanelPhysicalPlan
where
    F: FnMut(&ModuleId) -> Option<PanelPreferredSize>,
{
    let is_notification_center = matches!(
        &input.tree.content,
        PanelContent::ExternalProvider(provider) if provider.provider_name == "notifications"
    );
    // The Center is a reading surface even while loading or showing settings.
    // Its geometry must not depend on the current provider document.
    let hint = if is_notification_center {
        PanelContentSizeHint {
            preferred_width: Some(NOTIFICATION_CENTER_WIDTH),
            fill_height: true,
            ..Default::default()
        }
    } else if input.bar.uses_drawer() {
        PanelContentSizeHint {
            preferred_width: Some(DEFAULT_PANEL_FALLBACK_WIDTH),
            fill_edge_span: true,
            ..Default::default()
        }
    } else {
        panel_content_size_hint(&mut input)
    };
    let mut hint = hint;
    if let Some(width) = hint.preferred_width.as_mut() {
        *width = (*width as f32 + input.ui.padding.horizontal() - 32.0
            + input.ui.margin.horizontal())
        .max(1.0)
        .ceil() as u32;
    }
    if let Some(height) = hint.preferred_height.as_mut() {
        *height = (*height as f32 + input.ui.padding.vertical() - 32.0
            + input.ui.margin.vertical()
            + crate::theme::panel_header_height() as f32
            - 56.0)
            .max(1.0)
            .ceil() as u32;
    }
    let fill_width = hint.fill_edge_span && input.bar.edge.is_horizontal();
    let fill_height = hint.fill_height || (hint.fill_edge_span && !input.bar.edge.is_horizontal());
    let width = resolve_panel_dimension(
        input.bar.panel.width,
        input.bar.panel.max_width,
        hint.preferred_width,
        DEFAULT_PANEL_FALLBACK_WIDTH,
        fill_width,
        input.tree.screen_size.0,
    );
    let height = resolve_panel_dimension(
        input.bar.panel.height,
        input.bar.panel.max_height,
        hint.preferred_height,
        DEFAULT_PANEL_FALLBACK_HEIGHT,
        fill_height,
        // A horizontal bar reports its thickness, not the output height. Let
        // layer-shell fill the usable area, or use an explicit configured cap.
        if input.bar.edge.is_horizontal() {
            0
        } else {
            input.tree.screen_size.1
        },
    );
    let placement = crate::surface::placement::centered_position(
        &input.bar.edge,
        // Even a height-capped drawer stays at the start of the usable edge.
        // Switching modules must not move it to the new module's position.
        if input.bar.uses_drawer() || (hint.fill_height && !input.bar.edge.is_horizontal()) {
            None
        } else {
            input.tree.anchor
        },
        Size {
            width: width.value(),
            height: height.value(),
        },
        Size {
            width: input.tree.screen_size.0,
            height: input.tree.screen_size.1,
        },
        input.surface_origin,
        input.bar.collapsed_width,
        0,
    );

    PanelPhysicalPlan {
        edge: input.bar.edge.clone(),
        edge_offset: {
            let (near, inner, _, _) = input.bar.outer_margins();
            let occupied = near + input.bar.collapsed_width + inner;
            let reserve = if input.bar.reserve_space { occupied } else { 0 };
            let desired = if input.bar.drawer_pushes_bar() {
                near
            } else {
                occupied
            };
            desired as i32 - reserve as i32
        },
        width,
        height,
        placement: PanelPlacement {
            position: placement.position,
            size: placement.size,
            screen: placement.screen,
        },
        output_name: if input.tree.origin_output_name.is_empty() {
            None
        } else {
            Some(input.tree.origin_output_name.clone())
        },
    }
}

fn panel_content_size_hint<F>(input: &mut PanelSizingInput<'_, F>) -> PanelContentSizeHint
where
    F: FnMut(&ModuleId) -> Option<PanelPreferredSize>,
{
    match &input.tree.content {
        PanelContent::Menu(_) | PanelContent::DmenuRequest => {
            let preferred = panel_menu_preferred_size(&input.menu, input.menu_row_extent);
            PanelContentSizeHint {
                preferred_width: Some(preferred.width),
                preferred_height: Some(preferred.height),
                fill_edge_span: true,
                ..Default::default()
            }
        }
        PanelContent::ExternalProvider(_) => PanelContentSizeHint {
            preferred_width: Some(DEFAULT_PANEL_FALLBACK_WIDTH),
            // Providers host lists and forms that need a useful reading area even
            // when their first document is only a loading or empty state. Keep the
            // viewport stable as content arrives. A vertical bar's configured
            // surface height gives us its available edge span; a horizontal bar's
            // height is only its thickness, not the output height.
            preferred_height: Some(
                if !input.bar.edge.is_horizontal() && input.tree.screen_size.1 > 0 {
                    DEFAULT_PANEL_FALLBACK_HEIGHT.min(input.tree.screen_size.1)
                } else {
                    DEFAULT_PANEL_FALLBACK_HEIGHT
                },
            ),
            ..Default::default()
        },
        PanelContent::ModulePanel(id) => (input.module_detail_preferred_size)(id)
            .map(panel_module_window_size)
            .map(|preferred| PanelContentSizeHint {
                preferred_width: Some(preferred.width),
                preferred_height: Some(preferred.height),
                ..Default::default()
            })
            .unwrap_or_default(),
        PanelContent::BarPanel => bar_panel_preferred_size(
            input.panel_module_layout,
            &mut input.module_detail_preferred_size,
        )
        .map(|preferred| PanelContentSizeHint {
            preferred_width: Some(preferred.width),
            preferred_height: Some(preferred.height),
            fill_edge_span: true,
            ..Default::default()
        })
        .unwrap_or(PanelContentSizeHint {
            fill_edge_span: true,
            ..Default::default()
        }),
    }
}

fn resolve_panel_dimension(
    exact: Option<u32>,
    max: Option<u32>,
    preferred: Option<u32>,
    fallback: u32,
    fill_requested: bool,
    span: u32,
) -> PlannedDimension {
    if let Some(exact) = exact {
        return PlannedDimension::Exact(exact);
    }

    if fill_requested {
        if let Some(max) = max
            && (span == 0 || max < span)
        {
            return PlannedDimension::Exact(max);
        }
        return PlannedDimension::Fill { span };
    }

    let size = preferred.unwrap_or(fallback);
    PlannedDimension::Exact(max.map(|max| size.min(max)).unwrap_or(size))
}

fn panel_module_entry_size(content: PanelPreferredSize) -> PanelPreferredSize {
    PanelPreferredSize {
        width: content.width + PANEL_MODULE_FRAME_H_PADDING,
        height: content.height + PANEL_MODULE_FRAME_V_PADDING,
    }
}

fn panel_module_window_size(content: PanelPreferredSize) -> PanelPreferredSize {
    PanelPreferredSize {
        width: content.width + PANEL_OUTER_PADDING,
        height: content.height + PANEL_OUTER_PADDING + canopa_ui::panel::HEADER_HEIGHT,
    }
}

fn bar_panel_preferred_size(
    layout: &[crate::surface::panel::PanelModuleEntry],
    module_detail_preferred_size: &mut impl FnMut(&ModuleId) -> Option<PanelPreferredSize>,
) -> Option<PanelPreferredSize> {
    let mut entries = Vec::new();
    for entry in layout {
        let content_size = module_detail_preferred_size(&entry.id)?;
        entries.push(panel_module_entry_size(content_size));
    }
    panel_module_list_window_size(&entries)
}

fn panel_module_list_window_size(entries: &[PanelPreferredSize]) -> Option<PanelPreferredSize> {
    if entries.is_empty() {
        return None;
    }

    let width = entries.iter().map(|entry| entry.width).max().unwrap_or(0) + PANEL_OUTER_PADDING;
    let height = entries.iter().map(|entry| entry.height).sum::<u32>()
        + PANEL_ENTRY_SPACING * entries.len().saturating_sub(1) as u32
        + PANEL_OUTER_PADDING
        + canopa_ui::panel::HEADER_HEIGHT;

    Some(PanelPreferredSize { width, height })
}

fn panel_menu_preferred_size(
    menu: &super::foreground::PanelMenuRenderState,
    row_extent: f32,
) -> PanelPreferredSize {
    let prompt = menu.prompt.as_deref().unwrap_or("Search");
    let prompt_width = crate::theme::text_width(prompt, crate::theme::body_size()).ceil() as u32;
    let max_entry_width = menu
        .entries
        .iter()
        .map(|entry| {
            crate::theme::text_width(&entry.label, crate::theme::body_size()).ceil() as u32
        })
        .max()
        .unwrap_or(0);
    let icon_width = if menu.show_icons {
        PANEL_MENU_ICON_SLOT
    } else {
        0
    };
    let submenu_width = if menu.entries.iter().any(|entry| entry.is_submenu) {
        PANEL_MENU_SUBMENU_SLOT
    } else {
        0
    };
    let row_width = max_entry_width
        + PANEL_MENU_ROW_H_PADDING
        + PANEL_MENU_LIST_PADDING
        + icon_width
        + submenu_width;
    let search_width = prompt_width + PANEL_MENU_ROW_H_PADDING + PANEL_MENU_LIST_PADDING;
    let width = row_width.max(search_width).max(PANEL_MENU_MIN_WIDTH);
    let row_extent = row_extent.ceil() as u32;
    let height = canopa_ui::panel::HEADER_HEIGHT
        + (crate::theme::body_size() * 1.3
            + crate::theme::ui(canopa_theme::UiKind::Input)
                .padding
                .vertical())
        .ceil()
        .max(PANEL_MENU_SEARCH_HEIGHT as f32) as u32
        + PANEL_MENU_LIST_PADDING
        + row_extent * menu.entries.len() as u32;

    PanelPreferredSize { width, height }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::app::panel_menu_session::{PanelMenuEntry, PanelMenuEntryTarget};
    use crate::modules::Zone;
    use crate::surface::panel::{PanelContent, PanelModuleEntry, PanelOrigin};

    fn wid() -> iced::window::Id {
        iced::window::Id::unique()
    }

    fn tree(content: PanelContent, screen_size: (u32, u32)) -> PanelTree {
        PanelTree {
            origin: PanelOrigin::Bar,
            content,
            origin_window: wid(),
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: None,
            screen_size,
        }
    }

    fn empty_menu() -> super::super::foreground::PanelMenuRenderState {
        super::super::foreground::PanelMenuRenderState {
            entries: Arc::from([]),
            show_icons: false,
            prompt: None,
            search_query: String::new(),
            focused_index: None,
            can_go_back: false,
            scroll_y: 0.0,
            viewport_height: 0.0,
        }
    }

    fn plan_with<F>(
        bar: &BarConfig,
        tree: &PanelTree,
        panel_module_layout: &[PanelModuleEntry],
        menu: super::super::foreground::PanelMenuRenderState,
        module_detail_preferred_size: F,
    ) -> PanelPhysicalPlan
    where
        F: FnMut(&ModuleId) -> Option<PanelPreferredSize>,
    {
        panel_physical_plan(PanelSizingInput {
            bar,
            ui: crate::theme::ui(canopa_theme::UiKind::Panel),
            menu_row_extent: 45.0,
            tree,
            surface_origin: Point::default(),
            panel_module_layout,
            menu,
            module_detail_preferred_size,
        })
    }

    #[test]
    fn panel_menu_fills_horizontal_edge_span() {
        let bar = BarConfig {
            edge: Edge::Top,
            ..BarConfig::default()
        };
        let tree = tree(PanelContent::Menu("main".into()), (1920, 48));

        let plan = plan_with(&bar, &tree, &[], empty_menu(), |_| None);

        assert_eq!(plan.width, PlannedDimension::Fill { span: 1920 });
        assert_eq!(plan.height, PlannedDimension::Exact(56 + 38 + 32));
        assert_eq!(plan.layer_size(), (0, 126));
    }

    #[test]
    fn side_panel_menu_fills_vertical_edge_span() {
        let bar = BarConfig {
            edge: Edge::Left,
            ..BarConfig::default()
        };
        let tree = tree(PanelContent::Menu("main".into()), (48, 1080));

        let plan = plan_with(&bar, &tree, &[], empty_menu(), |_| None);

        assert_eq!(plan.width, PlannedDimension::Exact(240));
        assert_eq!(plan.height, PlannedDimension::Fill { span: 1080 });
        assert_eq!(plan.layer_size(), (240, 0));
    }

    #[test]
    fn exact_dimension_overrides_fill_sizing() {
        let mut bar = BarConfig {
            edge: Edge::Top,
            ..BarConfig::default()
        };
        bar.panel.width = Some(500);
        let tree = tree(PanelContent::Menu("main".into()), (1920, 48));

        let plan = plan_with(&bar, &tree, &[], empty_menu(), |_| None);

        assert_eq!(plan.width, PlannedDimension::Exact(500));
        assert_eq!(plan.height, PlannedDimension::Exact(126));
        assert_eq!(plan.layer_size(), (500, 126));
    }

    #[test]
    fn max_dimension_caps_fill_sizing() {
        let mut bar = BarConfig {
            edge: Edge::Top,
            ..BarConfig::default()
        };
        bar.panel.max_width = Some(800);
        let tree = tree(PanelContent::Menu("main".into()), (1920, 48));

        let plan = plan_with(&bar, &tree, &[], empty_menu(), |_| None);

        assert_eq!(plan.width, PlannedDimension::Exact(800));
        assert_eq!(plan.layer_size(), (800, 126));
    }

    #[test]
    fn module_panel_uses_module_preferred_size_with_shared_chrome() {
        let bar = BarConfig::default();
        let id = ModuleId::new("clock");
        let tree = tree(PanelContent::ModulePanel(id.clone()), (1920, 48));

        let plan = plan_with(&bar, &tree, &[], empty_menu(), |requested| {
            (requested == &id).then_some(PanelPreferredSize {
                width: 300,
                height: 260,
            })
        });

        assert_eq!(plan.width, PlannedDimension::Exact(332));
        assert_eq!(plan.height, PlannedDimension::Exact(348));
    }

    #[test]
    fn provider_popovers_have_room_for_lists_and_forms_on_every_edge() {
        use crate::surface::panel::ExternalPanelContent;
        for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
            let screen_size = if edge.is_horizontal() {
                (1920, 48)
            } else {
                (48, 1080)
            };
            let bar = BarConfig {
                edge,
                ..BarConfig::default()
            };
            for provider in ["wifi", "custom"] {
                let tree = tree(
                    PanelContent::ExternalProvider(ExternalPanelContent::failure(
                        provider, "Loading",
                    )),
                    screen_size,
                );
                let plan = plan_with(&bar, &tree, &[], empty_menu(), |_| None);
                assert_eq!(plan.layer_size(), (360, 600));
            }
        }
    }

    #[test]
    fn notification_center_is_wider_and_fills_height_on_every_edge() {
        use crate::surface::panel::ExternalPanelContent;
        for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
            for presentation in [
                crate::config::PanelPresentation::Auto,
                crate::config::PanelPresentation::Drawer,
            ] {
                let mut bar = BarConfig {
                    edge: edge.clone(),
                    ..BarConfig::default()
                };
                bar.panel.presentation = presentation;
                let screen_size = if edge.is_horizontal() {
                    (1920, 48)
                } else {
                    (48, 1080)
                };
                let mut tree = tree(
                    PanelContent::ExternalProvider(ExternalPanelContent::failure(
                        "notifications",
                        "Loading",
                    )),
                    screen_size,
                );
                tree.anchor = Some(iced::Rectangle::new(
                    iced::Point::new(900.0, 980.0),
                    iced::Size::new(48.0, 30.0),
                ));
                let plan = plan_with(&bar, &tree, &[], empty_menu(), |_| None);
                assert_eq!(plan.layer_size(), (480, 0));
                if !edge.is_horizontal() {
                    assert_eq!(plan.placement.position.y, 0);
                }
                // A top/bottom bar's thickness is not the available height.
                bar.panel.max_width = Some(420);
                bar.panel.max_height = Some(500);
                assert_eq!(
                    plan_with(&bar, &tree, &[], empty_menu(), |_| None).layer_size(),
                    (420, 500)
                );
                bar.panel.width = Some(400);
                bar.panel.height = Some(450);
                assert_eq!(
                    plan_with(&bar, &tree, &[], empty_menu(), |_| None).layer_size(),
                    (400, 450)
                );
            }
        }
    }

    #[test]
    fn provider_popover_height_respects_short_side_outputs_and_configured_limits() {
        use crate::surface::panel::ExternalPanelContent;
        let mut bar = BarConfig::default();
        let mut tree = tree(
            PanelContent::ExternalProvider(ExternalPanelContent::failure("wifi", "Loading")),
            (48, 480),
        );
        tree.anchor = Some(iced::Rectangle::new(
            iced::Point::new(0.0, 440.0),
            iced::Size::new(48.0, 30.0),
        ));
        let plan = plan_with(&bar, &tree, &[], empty_menu(), |_| None);
        assert_eq!(plan.layer_size(), (360, 480));
        assert_eq!(plan.placement.position.y, 0);
        bar.panel.max_height = Some(400);
        assert_eq!(
            plan_with(&bar, &tree, &[], empty_menu(), |_| None).layer_size(),
            (360, 400)
        );
        bar.panel.height = Some(300);
        assert_eq!(
            plan_with(&bar, &tree, &[], empty_menu(), |_| None).layer_size(),
            (360, 300)
        );
        bar.panel.height = None;
        bar.panel.max_height = None;
        tree.screen_size = (0, 0);
        assert_eq!(
            plan_with(&bar, &tree, &[], empty_menu(), |_| None).layer_size(),
            (360, 600)
        );
    }

    #[test]
    fn drawer_has_stable_geometry_across_modules_menus_and_provider_content() {
        use crate::config::PanelPresentation;
        use crate::surface::panel::ExternalPanelContent;

        for edge in [Edge::Left, Edge::Right] {
            let mut bar = BarConfig {
                edge,
                ..BarConfig::default()
            };
            bar.panel.presentation = PanelPresentation::Drawer;
            for content in [
                PanelContent::ModulePanel(ModuleId::new("clock")),
                PanelContent::ModulePanel(ModuleId::new("volume")),
                PanelContent::Menu("main".into()),
                PanelContent::DmenuRequest,
                PanelContent::BarPanel,
                PanelContent::ExternalProvider(ExternalPanelContent::failure("wifi", "Offline")),
            ] {
                let mut tree = tree(content, (1920, 1080));
                for y in [10.0, 980.0] {
                    tree.anchor = Some(iced::Rectangle {
                        x: 0.0,
                        y,
                        width: 48.0,
                        height: 30.0,
                    });
                    let plan = plan_with(&bar, &tree, &[], empty_menu(), |_| {
                        Some(PanelPreferredSize {
                            width: 800,
                            height: 120,
                        })
                    });
                    assert_eq!(plan.layer_size(), (360, 0));
                    assert_eq!(plan.height, PlannedDimension::Fill { span: 1080 });
                    assert_eq!(plan.placement.position.y, 0);
                    assert_eq!(
                        plan.placement.position.x,
                        if matches!(bar.edge, Edge::Left) {
                            48
                        } else {
                            1512
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn capped_drawer_stays_at_usable_edge_start_and_respects_dimensions() {
        let mut bar = BarConfig::default();
        bar.panel.presentation = crate::config::PanelPresentation::Drawer;
        bar.panel.max_width = Some(320);
        bar.panel.max_height = Some(500);
        let mut tree = tree(
            PanelContent::ModulePanel(ModuleId::new("clock")),
            (1920, 1000),
        );
        tree.anchor = Some(iced::Rectangle {
            x: 0.0,
            y: 900.0,
            width: 48.0,
            height: 30.0,
        });
        let plan = panel_physical_plan(PanelSizingInput {
            bar: &bar,
            ui: crate::theme::ui(canopa_theme::UiKind::Panel),
            menu_row_extent: 45.0,
            tree: &tree,
            surface_origin: Point { x: 0, y: 40 },
            panel_module_layout: &[],
            menu: empty_menu(),
            module_detail_preferred_size: |_| None,
        });
        assert_eq!(plan.layer_size(), (320, 500));
        assert_eq!(plan.placement.position.y, 40);
        bar.panel.width = Some(300);
        bar.panel.height = Some(400);
        let plan = plan_with(&bar, &tree, &[], empty_menu(), |_| None);
        assert_eq!(plan.layer_size(), (300, 400));
        assert_eq!(plan.placement.position.y, 0);
    }

    #[test]
    fn drawer_preference_keeps_horizontal_module_panels_content_sized() {
        for edge in [Edge::Top, Edge::Bottom] {
            let mut bar = BarConfig {
                edge,
                ..BarConfig::default()
            };
            bar.panel.presentation = crate::config::PanelPresentation::Drawer;
            let tree = tree(
                PanelContent::ModulePanel(ModuleId::new("clock")),
                (1920, 1080),
            );
            let plan = plan_with(&bar, &tree, &[], empty_menu(), |_| {
                Some(PanelPreferredSize {
                    width: 300,
                    height: 260,
                })
            });
            assert_eq!(plan.layer_size(), (332, 348));
        }
    }

    #[test]
    fn bar_panel_aggregates_panel_module_entry_sizes() {
        let bar = BarConfig {
            edge: Edge::Top,
            ..BarConfig::default()
        };
        let first = ModuleId::new("first");
        let second = ModuleId::new("second");
        let tree = tree(PanelContent::BarPanel, (1920, 48));
        let layout = vec![
            PanelModuleEntry {
                id: first.clone(),
                zone: Zone::Start,
                focused: false,
            },
            PanelModuleEntry {
                id: second.clone(),
                zone: Zone::End,
                focused: false,
            },
        ];

        let plan = plan_with(&bar, &tree, &layout, empty_menu(), |requested| {
            if requested == &first {
                Some(PanelPreferredSize {
                    width: 100,
                    height: 20,
                })
            } else if requested == &second {
                Some(PanelPreferredSize {
                    width: 180,
                    height: 30,
                })
            } else {
                None
            }
        });

        assert_eq!(plan.width, PlannedDimension::Fill { span: 1920 });
        assert_eq!(
            plan.height,
            PlannedDimension::Exact(20 + 16 + 30 + 16 + 8 + 32 + 56)
        );
    }

    #[test]
    fn panel_menu_preferred_size_accounts_for_prompt_icons_and_submenus() {
        let menu = super::super::foreground::PanelMenuRenderState {
            entries: Arc::from([
                PanelMenuEntry {
                    label: "Browser".into(),
                    icon: Some("firefox".into()),
                    path: vec![0],
                    target: PanelMenuEntryTarget::StaticPath(vec![0]),
                    is_submenu: false,
                },
                PanelMenuEntry {
                    label: "Development Tools".into(),
                    icon: None,
                    path: vec![1],
                    target: PanelMenuEntryTarget::StaticPath(vec![1]),
                    is_submenu: true,
                },
            ]),
            show_icons: true,
            prompt: Some("Run".into()),
            search_query: String::new(),
            focused_index: None,
            can_go_back: false,
            scroll_y: 0.0,
            viewport_height: 0.0,
        };

        let preferred = panel_menu_preferred_size(&menu, 45.0);

        assert_eq!(preferred.width, 240);
        assert_eq!(preferred.height, 56 + 38 + 32 + 45 * 2);
    }
}
