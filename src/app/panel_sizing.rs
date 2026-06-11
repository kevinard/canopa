use crate::config::{BarConfig, Edge};
use crate::modules::ModuleId;
use crate::surface::panel::{PanelContent, PanelPreferredSize, PanelTree};
use crate::surface::placement::{Placement, Point, Size};

pub(super) const MENU_ROW_EXTENT: f32 = 45.0;

const DEFAULT_PANEL_FALLBACK_WIDTH: u32 = 360;
const DEFAULT_PANEL_FALLBACK_HEIGHT: u32 = 600;
const PANEL_OUTER_PADDING: u32 = 16;
const PANEL_MODULE_FRAME_H_PADDING: u32 = 20;
const PANEL_MODULE_FRAME_V_PADDING: u32 = 16;
const PANEL_ENTRY_SPACING: u32 = 8;
const PANEL_MENU_MIN_WIDTH: u32 = 240;
const PANEL_MENU_SEARCH_HEIGHT: u32 = 38;
const PANEL_MENU_LIST_PADDING: u32 = 16;
const PANEL_MENU_ROW_H_PADDING: u32 = 20;
const PANEL_MENU_CHAR_WIDTH: u32 = 8;
const PANEL_MENU_ICON_SLOT: u32 = 26;
const PANEL_MENU_SUBMENU_SLOT: u32 = 16;

pub(super) struct PanelSizingInput<'a, F>
where
    F: FnMut(&ModuleId) -> Option<PanelPreferredSize>,
{
    pub(super) bar: &'a BarConfig,
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
}

pub(super) fn panel_physical_plan<F>(mut input: PanelSizingInput<'_, F>) -> PanelPhysicalPlan
where
    F: FnMut(&ModuleId) -> Option<PanelPreferredSize>,
{
    let hint = panel_content_size_hint(&mut input);
    let fill_width = hint.fill_edge_span && input.bar.edge.is_horizontal();
    let fill_height = hint.fill_edge_span && !input.bar.edge.is_horizontal();
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
        input.tree.screen_size.1,
    );
    let placement = crate::surface::placement::centered_position(
        &input.bar.edge,
        input.tree.anchor,
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
            let preferred = panel_menu_preferred_size(&input.menu);
            PanelContentSizeHint {
                preferred_width: Some(preferred.width),
                preferred_height: Some(preferred.height),
                fill_edge_span: true,
            }
        }
        PanelContent::ModulePanel(id) => (input.module_detail_preferred_size)(id)
            .map(panel_module_window_size)
            .map(|preferred| PanelContentSizeHint {
                preferred_width: Some(preferred.width),
                preferred_height: Some(preferred.height),
                fill_edge_span: false,
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
    panel_module_list_window_size(&[panel_module_entry_size(content)])
        .expect("single module detail produces one panel entry")
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
        + PANEL_OUTER_PADDING;

    Some(PanelPreferredSize { width, height })
}

fn panel_menu_preferred_size(menu: &super::foreground::PanelMenuRenderState) -> PanelPreferredSize {
    let prompt_chars = menu.prompt.as_deref().unwrap_or("Search").chars().count() as u32;
    let max_entry_chars = menu
        .entries
        .iter()
        .map(|entry| entry.label.chars().count() as u32)
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
    let row_width = max_entry_chars * PANEL_MENU_CHAR_WIDTH
        + PANEL_MENU_ROW_H_PADDING
        + PANEL_MENU_LIST_PADDING
        + icon_width
        + submenu_width;
    let search_width =
        prompt_chars * PANEL_MENU_CHAR_WIDTH + PANEL_MENU_ROW_H_PADDING + PANEL_MENU_LIST_PADDING;
    let width = row_width.max(search_width).max(PANEL_MENU_MIN_WIDTH);
    let row_extent = MENU_ROW_EXTENT.ceil() as u32;
    let height =
        PANEL_MENU_SEARCH_HEIGHT + PANEL_MENU_LIST_PADDING + row_extent * menu.entries.len() as u32;

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
            scroll_y: 0.0,
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
        assert_eq!(plan.height, PlannedDimension::Exact(38 + 16));
        assert_eq!(plan.layer_size(), (0, 54));
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
        assert_eq!(plan.height, PlannedDimension::Exact(54));
        assert_eq!(plan.layer_size(), (500, 54));
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
        assert_eq!(plan.layer_size(), (800, 54));
    }

    #[test]
    fn module_panel_uses_module_preferred_size_with_frame() {
        let bar = BarConfig::default();
        let id = ModuleId::new("clock");
        let tree = tree(PanelContent::ModulePanel(id.clone()), (1920, 48));

        let plan = plan_with(&bar, &tree, &[], empty_menu(), |requested| {
            (requested == &id).then_some(PanelPreferredSize {
                width: 300,
                height: 260,
            })
        });

        assert_eq!(plan.width, PlannedDimension::Exact(336));
        assert_eq!(plan.height, PlannedDimension::Exact(292));
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
            PlannedDimension::Exact(20 + 16 + 30 + 16 + 8 + 16)
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
            scroll_y: 0.0,
        };

        let preferred = panel_menu_preferred_size(&menu);

        assert_eq!(preferred.width, 240);
        assert_eq!(preferred.height, 38 + 16 + 45 * 2);
    }
}
