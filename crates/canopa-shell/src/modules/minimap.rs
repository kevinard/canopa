//! Minimap: the scrolling strip of the workspace shown on the bar's output,
//! drawn to scale. Each column is a bar of its tiled windows; the focused
//! window is accented and its column stands out. A click focuses a window,
//! and scrolling over the strip moves focus between columns. Niri owns the
//! layout; the Shell only draws what Niri reports.
//!
//! When Niri reports where tiled windows sit in the view (the Canopa fork's
//! `canopa/viewport-ipc`; upstream Niri 26.04 reports it for floating windows
//! only), a frame marks the part of the strip on screen and columns outside
//! it are dimmer. Floating windows are counted, not drawn.
use crate::modules::{InteractionEvent, ModuleId, ModuleKind, ModuleState};
use crate::niri::NiriState;

/// The strip of one workspace.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MinimapState {
    /// Columns from left to right.
    pub columns: Vec<MinimapColumn>,
    /// The height of the workspace in logical pixels, which sets the scale.
    pub workspace_height: f64,
    /// Floating windows on the workspace, which the strip does not draw.
    pub floating: usize,
    /// The width of the workspace view in logical pixels, when known.
    pub view_width: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MinimapColumn {
    /// The widest tile of the column, in logical pixels.
    pub width: f64,
    /// Tiles from top to bottom.
    pub tiles: Vec<MinimapTile>,
    /// The column holds the workspace's active window.
    pub active: bool,
    /// The column's left edge in the workspace view, when Niri reports it:
    /// negative or past the view's width when it is off screen.
    pub view_x: Option<f64>,
    /// The edge of the view the column is pinned to; it stays on screen while
    /// the rest of the strip scrolls.
    pub pinned: Option<niri_ipc::PinSide>,
}

impl MinimapColumn {
    /// The part of the column on screen, as fractions of its width, or
    /// `None` when it is off screen. Columns without a position count as
    /// visible.
    pub(crate) fn visible_range(&self, view_width: Option<f64>) -> Option<(f32, f32)> {
        let (Some(x), Some(view), true) = (self.view_x, view_width, self.width > 0.0) else {
            return Some((0.0, 1.0));
        };
        let start = (-x / self.width).clamp(0.0, 1.0);
        let end = ((view - x) / self.width).clamp(0.0, 1.0);
        (end - start > 0.01).then_some((start as f32, end as f32))
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MinimapTile {
    pub id: u64,
    /// The tile's height in logical pixels.
    pub height: f64,
    pub focused: bool,
    pub urgent: bool,
    pub title: Option<String>,
}

impl MinimapState {
    /// The 1-based position of the active column, and the column count.
    pub(crate) fn active_column(&self) -> Option<(usize, usize)> {
        self.columns
            .iter()
            .position(|column| column.active)
            .map(|index| (index + 1, self.columns.len()))
    }

    fn active_title(&self) -> Option<&str> {
        let column = self.columns.iter().find(|column| column.active)?;
        column
            .tiles
            .iter()
            .find(|tile| tile.focused)
            .or_else(|| column.tiles.first())
            .and_then(|tile| tile.title.as_deref())
    }
}

#[derive(Debug, Clone)]
pub enum MinimapEvent {
    /// Focus a window from the strip.
    FocusWindow(u64),
    /// Move focus to the next column, or the previous one.
    FocusColumn { right: bool },
}

/// The strip of the workspace active on `output`, or of the focused
/// workspace when the bar surface has no output name.
pub(crate) fn strip(niri: &NiriState, output: &str) -> Option<MinimapState> {
    let workspace = niri.workspaces.iter().find(|workspace| {
        if output.is_empty() {
            workspace.is_focused
        } else {
            workspace.is_active && workspace.output.as_deref() == Some(output)
        }
    })?;

    let mut tiled: Vec<(usize, usize, &niri_ipc::Window)> = Vec::new();
    let mut floating = 0;
    for window in niri.windows.values() {
        if window.workspace_id != Some(workspace.id) {
            continue;
        }
        match window.layout.pos_in_scrolling_layout {
            Some((column, tile)) => tiled.push((column, tile, window)),
            None => floating += 1,
        }
    }
    tiled.sort_by_key(|(column, tile, window)| (*column, *tile, window.id));

    let mut columns: Vec<MinimapColumn> = Vec::new();
    let mut last_column = None;
    for (column, _, window) in tiled {
        if last_column != Some(column) {
            columns.push(MinimapColumn::default());
            last_column = Some(column);
        }
        let current = columns.last_mut().expect("column pushed above");
        let (width, height) = window.layout.tile_size;
        current.width = current.width.max(width);
        current.pinned = current.pinned.or(window.layout.pinned);
        if let Some((x, _)) = window.layout.tile_pos_in_workspace_view {
            current.view_x = Some(current.view_x.map_or(x, |known: f64| known.min(x)));
        }
        current.active |= workspace.active_window_id == Some(window.id);
        current.tiles.push(MinimapTile {
            id: window.id,
            height,
            focused: niri.focused_window_id == Some(window.id),
            urgent: window.is_urgent,
            title: window.title.clone().or_else(|| window.app_id.clone()),
        });
    }

    // Niri fills the working area's height with each column's tiles, so the
    // tallest column stands in when the output's size is not known yet.
    let workspace_height = workspace
        .output
        .as_deref()
        .and_then(|output| niri.output_sizes.get(output))
        .map(|(_, height)| *height)
        .filter(|height| *height > 0.0)
        .unwrap_or_else(|| {
            columns
                .iter()
                .map(|column| column.tiles.iter().map(|tile| tile.height).sum::<f64>())
                .fold(0.0, f64::max)
        });

    let view_width = workspace
        .output
        .as_deref()
        .and_then(|output| niri.output_sizes.get(output))
        .map(|(width, _)| *width)
        .filter(|width| *width > 0.0);

    Some(MinimapState {
        columns,
        workspace_height,
        floating,
        view_width,
    })
}

/// Space between columns and between the tiles of a column; a pinned
/// column stands further apart from the strip it does not scroll with.
const COLUMN_GAP: f32 = 2.0;
const PIN_GAP: f32 = 6.0;

/// The space after column `index`.
fn gap_after(columns: &[MinimapColumn], index: usize) -> f32 {
    let pinned_boundary = columns[index].pinned == Some(niri_ipc::PinSide::Left)
        || columns
            .get(index + 1)
            .is_some_and(|next| next.pinned == Some(niri_ipc::PinSide::Right));
    if pinned_boundary { PIN_GAP } else { COLUMN_GAP }
}

fn total_gaps(columns: &[MinimapColumn]) -> f32 {
    (0..columns.len().saturating_sub(1))
        .map(|index| gap_after(columns, index))
        .sum()
}
const TILE_GAP: f32 = 1.0;
const MIN_EXTENT: f32 = 2.0;

/// Column lengths along the strip, in bar pixels: to scale with the
/// workspace's height drawn at `size`, shrunk together to fit `max_length`.
pub(crate) fn column_lengths(state: &MinimapState, size: f32, max_length: f32) -> Vec<f32> {
    if state.columns.is_empty() {
        return Vec::new();
    }
    let scale = if state.workspace_height > 0.0 {
        size / state.workspace_height as f32
    } else {
        0.0
    };
    let mut lengths: Vec<f32> = state
        .columns
        .iter()
        .map(|column| (column.width as f32 * scale).max(MIN_EXTENT + 1.0))
        .collect();
    let gaps = total_gaps(&state.columns);
    let total: f32 = lengths.iter().sum::<f32>() + gaps;
    if max_length > 0.0 && total > max_length {
        let factor = ((max_length - gaps).max(0.0)) / (total - gaps);
        for length in &mut lengths {
            *length = (*length * factor).max(MIN_EXTENT);
        }
    }
    lengths
}

/// Tile extents across the strip: the column's tiles share `size` by height.
pub(crate) fn tile_extents(column: &MinimapColumn, size: f32) -> Vec<f32> {
    let count = column.tiles.len();
    if count == 0 {
        return Vec::new();
    }
    let available = (size - TILE_GAP * (count - 1) as f32).max(MIN_EXTENT * count as f32);
    let total: f64 = column.tiles.iter().map(|tile| tile.height.max(0.0)).sum();
    column
        .tiles
        .iter()
        .map(|tile| {
            let share = if total > 0.0 {
                (tile.height.max(0.0) / total) as f32
            } else {
                1.0 / count as f32
            };
            (available * share).max(MIN_EXTENT)
        })
        .collect()
}

pub(crate) struct MinimapModule {
    id: ModuleId,
    config: crate::config::MinimapConfig,
}

impl MinimapModule {
    pub(crate) fn new(id: ModuleId, config: crate::config::MinimapConfig) -> Self {
        Self { id, config }
    }

    /// The strip, centered across the bar.
    fn view(
        &self,
        state: &MinimapState,
        horizontal: bool,
    ) -> iced::Element<'static, crate::app::Message> {
        let strip = iced::widget::container(self.strip(state, horizontal));
        if horizontal {
            strip.center_y(iced::Length::Fill).into()
        } else {
            strip.center_x(iced::Length::Fill).into()
        }
    }

    fn strip(
        &self,
        state: &MinimapState,
        horizontal: bool,
    ) -> iced::Element<'static, crate::app::Message> {
        use iced::widget::{Space, column, container, mouse_area, row};
        let size = f32::from(self.config.size);
        let colors = crate::theme::palette();
        let lengths = column_lengths(state, size, self.config.max_length as f32);

        if lengths.is_empty() {
            // An empty workspace: the outline of one screen.
            let (width, height) = if horizontal {
                (size * 1.6, size)
            } else {
                (size, size * 1.6)
            };
            let outline = iced::Color {
                a: 0.35,
                ..colors.text
            };
            return container(Space::new().width(width).height(height))
                .style(move |_| container::Style {
                    border: iced::Border {
                        color: outline,
                        width: 1.0,
                        radius: 2.0.into(),
                    },
                    ..Default::default()
                })
                .into();
        }

        let span = visible_span(state, &lengths);
        let mut strip: Vec<iced::Element<'static, crate::app::Message>> = Vec::new();
        for (index, (column_state, length)) in state
            .columns
            .iter()
            .zip(lengths.iter().copied())
            .enumerate()
        {
            if index > 0 {
                let gap = gap_after(&state.columns, index - 1);
                strip.push(if horizontal {
                    Space::new().width(gap).into()
                } else {
                    Space::new().height(gap).into()
                });
            }
            let pinned = column_state.pinned.is_some();
            let on_screen = column_state.visible_range(state.view_width).is_some();
            let tiles = column_state
                .tiles
                .iter()
                .zip(tile_extents(column_state, size))
                .map(|(tile, extent)| {
                    let color = if tile.urgent {
                        colors.warning
                    } else if tile.focused {
                        colors.accent
                    } else if column_state.active {
                        iced::Color {
                            a: 0.7,
                            ..colors.text
                        }
                    } else if on_screen {
                        iced::Color {
                            a: 0.32,
                            ..colors.text
                        }
                    } else {
                        iced::Color {
                            a: 0.16,
                            ..colors.text
                        }
                    };
                    let (width, height) = if horizontal {
                        (length, extent)
                    } else {
                        (extent, length)
                    };
                    let block =
                        container(Space::new().width(width).height(height)).style(move |_| {
                            container::Style {
                                background: Some(iced::Background::Color(color)),
                                border: iced::Border {
                                    radius: 2.0.into(),
                                    // A pinned column is outlined.
                                    width: if pinned { 1.0 } else { 0.0 },
                                    color: iced::Color {
                                        a: 0.85,
                                        ..colors.text
                                    },
                                },
                                ..Default::default()
                            }
                        });
                    mouse_area(block)
                        .on_press(crate::app::Message::Minimap(MinimapEvent::FocusWindow(
                            tile.id,
                        )))
                        .interaction(iced::mouse::Interaction::Pointer)
                        .into()
                })
                .collect::<Vec<iced::Element<'static, crate::app::Message>>>();
            strip.push(if horizontal {
                column(tiles).spacing(TILE_GAP).into()
            } else {
                row(tiles).spacing(TILE_GAP).into()
            });
        }
        let strip: iced::Element<'static, crate::app::Message> = if horizontal {
            row(strip).align_y(iced::Alignment::Center).into()
        } else {
            column(strip).align_x(iced::Alignment::Center).into()
        };
        let Some((start, end)) = span else {
            return strip;
        };
        // The frame around the part on screen sits FRAME_INSET outside the
        // blocks; the strip is centered inside the same box.
        let across = size + 2.0 * FRAME_INSET;
        let outline = iced::Color {
            a: 0.75,
            ..colors.text
        };
        let frame = container(Space::new()).style(move |_| container::Style {
            border: iced::Border {
                color: outline,
                width: 1.0,
                radius: 3.0.into(),
            },
            ..Default::default()
        });
        let along = end - start + 2.0 * FRAME_INSET;
        if horizontal {
            iced::widget::stack![
                container(strip).padding([FRAME_INSET, FRAME_INSET]),
                row![Space::new().width(start), frame.width(along).height(across)],
            ]
            .into()
        } else {
            iced::widget::stack![
                container(strip).padding([FRAME_INSET, FRAME_INSET]),
                column![
                    Space::new().height(start),
                    frame.width(across).height(along)
                ],
            ]
            .into()
        }
    }
}

/// How far the visible-area frame sits outside the blocks.
const FRAME_INSET: f32 = 2.0;

/// The part of the strip on screen, in strip pixels from its start, when
/// Niri reports the view and part of the strip is off screen.
pub(crate) fn visible_span(state: &MinimapState, lengths: &[f32]) -> Option<(f32, f32)> {
    state.view_width?;
    if state.columns.iter().all(|column| column.view_x.is_none()) {
        return None;
    }
    let mut offset = 0.0;
    let mut span: Option<(f32, f32)> = None;
    let mut clipped = false;
    for (index, (column, length)) in state.columns.iter().zip(lengths).enumerate() {
        match column.visible_range(state.view_width) {
            Some((from, to)) => {
                clipped |= from > 0.0 || to < 1.0;
                let (start, end) = (offset + from * length, offset + to * length);
                span = Some(span.map_or((start, end), |(first, _)| (first, end)));
            }
            None => clipped = true,
        }
        if index + 1 < state.columns.len() {
            offset += length + gap_after(&state.columns, index);
        }
    }
    span.filter(|_| clipped)
}

impl crate::modules::Module for MinimapModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Minimap
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none()
    }

    fn interaction_handler(
        &self,
    ) -> Option<&dyn crate::modules::interaction::ModuleInteractionHandler> {
        Some(self)
    }
}

impl crate::modules::interaction::ModuleInteractionHandler for MinimapModule {
    fn on_interaction(&self, event: InteractionEvent) -> Option<crate::app::Message> {
        match event {
            InteractionEvent::ScrollUp => {
                Some(crate::app::Message::Minimap(MinimapEvent::FocusColumn {
                    right: false,
                }))
            }
            InteractionEvent::ScrollDown => {
                Some(crate::app::Message::Minimap(MinimapEvent::FocusColumn {
                    right: true,
                }))
            }
            _ => None,
        }
    }
}

impl crate::modules::render::ModuleRender for MinimapModule {
    fn has_content(&self, state: &ModuleState, _mode: crate::config::FormatMode) -> bool {
        matches!(state, ModuleState::Minimap(_))
    }

    fn render_bar(
        &self,
        state: &ModuleState,
        context: &crate::modules::render::BarRenderContext<'_>,
        _style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        match state {
            ModuleState::Minimap(state) => self.view(state, context.edge.is_horizontal()),
            _ => iced::widget::Space::new().into(),
        }
    }

    fn render_collapsed_with_style(
        &self,
        state: &ModuleState,
        _style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        match state {
            ModuleState::Minimap(state) => self.view(state, false),
            _ => iced::widget::Space::new().into(),
        }
    }

    fn render_panel_with_style(
        &self,
        state: &ModuleState,
        _style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        match state {
            ModuleState::Minimap(state) => self.view(state, true),
            _ => iced::widget::Space::new().into(),
        }
    }

    fn accessible_values(&self, state: &ModuleState) -> Vec<(&'static str, String)> {
        let ModuleState::Minimap(state) = state else {
            return Vec::new();
        };
        let mut values = vec![("columns", state.columns.len().to_string())];
        if let Some((column, _)) = state.active_column() {
            values.push(("column", column.to_string()));
        }
        if let Some(title) = state.active_title() {
            values.push(("title", title.to_owned()));
        }
        if state.floating > 0 {
            values.push(("floating", state.floating.to_string()));
        }
        if let Some(column) = state.columns.iter().find(|column| column.active)
            && let Some(side) = column.pinned
        {
            values.push((
                "pinned",
                match side {
                    niri_ipc::PinSide::Left => "left",
                    niri_ipc::PinSide::Right => "right",
                }
                .into(),
            ));
        }
        values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(
        id: u64,
        output: &str,
        active: bool,
        focused: bool,
        window: Option<u64>,
    ) -> niri_ipc::Workspace {
        niri_ipc::Workspace {
            id,
            idx: id as u8,
            name: None,
            output: Some(output.into()),
            is_urgent: false,
            is_active: active,
            is_focused: focused,
            active_window_id: window,
        }
    }

    fn window(
        id: u64,
        workspace: u64,
        pos: Option<(usize, usize)>,
        size: (f64, f64),
    ) -> niri_ipc::Window {
        niri_ipc::Window {
            id,
            title: Some(format!("Window {id}")),
            app_id: None,
            pid: None,
            parent_id: None,
            workspace_id: Some(workspace),
            is_focused: false,
            is_floating: pos.is_none(),
            is_urgent: false,
            layout: niri_ipc::WindowLayout {
                pos_in_scrolling_layout: pos,
                tile_size: size,
                window_size: (size.0 as i32, size.1 as i32),
                tile_pos_in_workspace_view: None,
                window_offset_in_tile: (0.0, 0.0),
                pinned: None,
            },
            focus_timestamp: None,
        }
    }

    fn niri(windows: Vec<niri_ipc::Window>) -> NiriState {
        NiriState {
            workspaces: vec![
                workspace(1, "eDP-1", true, true, Some(12)),
                workspace(2, "eDP-1", false, false, None),
                workspace(3, "DP-1", true, false, Some(30)),
            ],
            focused_window_id: Some(12),
            windows: windows
                .into_iter()
                .map(|window| (window.id, window))
                .collect(),
            connected: true,
            ..NiriState::default()
        }
    }

    #[test]
    fn strip_orders_columns_and_tiles_from_the_layout() {
        let state = niri(vec![
            window(13, 1, Some((2, 1)), (960.0, 1000.0)),
            window(11, 1, Some((1, 2)), (640.0, 495.0)),
            window(12, 1, Some((1, 1)), (640.0, 495.0)),
            window(14, 1, None, (400.0, 300.0)),
            window(20, 2, Some((1, 1)), (960.0, 1000.0)),
            window(30, 3, Some((1, 1)), (1920.0, 1000.0)),
        ]);

        let strip = strip(&state, "eDP-1").expect("strip");

        assert_eq!(strip.columns.len(), 2);
        let ids: Vec<Vec<u64>> = strip
            .columns
            .iter()
            .map(|column| column.tiles.iter().map(|tile| tile.id).collect())
            .collect();
        assert_eq!(ids, [vec![12, 11], vec![13]]);
        assert!(strip.columns[0].active);
        assert!(strip.columns[0].tiles[0].focused);
        assert!(!strip.columns[1].active);
        assert_eq!(strip.floating, 1);
        assert_eq!(strip.active_column(), Some((1, 2)));
        assert_eq!(strip.active_title(), Some("Window 12"));
        // Without output sizes, the tallest column sets the scale.
        assert_eq!(strip.workspace_height, 1000.0);
    }

    #[test]
    fn strip_follows_the_active_workspace_of_each_output() {
        let state = niri(vec![window(30, 3, Some((1, 1)), (1920.0, 1000.0))]);

        let other = strip(&state, "DP-1").expect("strip");

        assert_eq!(other.columns.len(), 1);
        assert!(other.columns[0].active);
        assert!(!other.columns[0].tiles[0].focused);
        assert!(strip(&state, "HDMI-A-1").is_none());
    }

    #[test]
    fn strip_without_an_output_name_shows_the_focused_workspace() {
        let state = niri(vec![window(12, 1, Some((1, 1)), (960.0, 1000.0))]);

        assert_eq!(strip(&state, "").expect("strip").columns.len(), 1);
    }

    #[test]
    fn output_height_sets_the_scale_when_known() {
        let mut state = niri(vec![window(12, 1, Some((1, 1)), (960.0, 500.0))]);
        state.output_sizes.insert("eDP-1".into(), (1920.0, 1080.0));

        assert_eq!(
            strip(&state, "eDP-1").expect("strip").workspace_height,
            1080.0
        );
    }

    #[test]
    fn column_lengths_keep_the_screen_aspect_and_fit_the_maximum() {
        let state = MinimapState {
            columns: vec![
                MinimapColumn {
                    width: 1000.0,
                    ..Default::default()
                },
                MinimapColumn {
                    width: 500.0,
                    ..Default::default()
                },
            ],
            workspace_height: 1000.0,
            floating: 0,
            view_width: None,
        };

        assert_eq!(column_lengths(&state, 20.0, 0.0), [20.0, 10.0]);
        let fitted = column_lengths(&state, 20.0, 17.0);
        assert_eq!(fitted, [10.0, 5.0]);
        assert_eq!(fitted.iter().sum::<f32>() + COLUMN_GAP, 17.0);
    }

    #[test]
    fn tile_extents_share_the_strip_by_height() {
        let column = MinimapColumn {
            width: 960.0,
            tiles: vec![
                MinimapTile {
                    height: 300.0,
                    ..Default::default()
                },
                MinimapTile {
                    height: 600.0,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        assert_eq!(tile_extents(&column, 19.0), [6.0, 12.0]);
    }

    #[test]
    fn the_frame_spans_the_columns_on_screen() {
        let column = |view_x| MinimapColumn {
            width: 500.0,
            view_x: Some(view_x),
            ..Default::default()
        };
        // A 1000-pixel view showing the second column, the left half of the
        // third and nothing of the first or fourth.
        let state = MinimapState {
            columns: vec![column(-520.0), column(0.0), column(750.0), column(1270.0)],
            workspace_height: 1000.0,
            floating: 0,
            view_width: Some(1000.0),
        };
        let lengths = [10.0, 10.0, 20.0, 10.0];
        assert_eq!(state.columns[0].visible_range(state.view_width), None);
        assert_eq!(
            state.columns[2].visible_range(state.view_width),
            Some((0.0, 0.5))
        );
        assert_eq!(visible_span(&state, &lengths), Some((12.0, 34.0)));

        // Everything on screen, or no positions from Niri: no frame.
        let all = MinimapState {
            columns: vec![column(0.0), column(500.0)],
            ..state.clone()
        };
        assert_eq!(visible_span(&all, &lengths), None);
        let unknown = MinimapState {
            columns: vec![MinimapColumn {
                width: 500.0,
                ..Default::default()
            }],
            ..state
        };
        assert_eq!(visible_span(&unknown, &lengths), None);
    }

    #[test]
    fn pinned_columns_stand_apart_from_the_strip() {
        let column = |pinned| MinimapColumn {
            width: 500.0,
            pinned,
            ..Default::default()
        };
        let columns = [
            column(Some(niri_ipc::PinSide::Left)),
            column(None),
            column(None),
            column(Some(niri_ipc::PinSide::Right)),
        ];
        assert_eq!(gap_after(&columns, 0), PIN_GAP);
        assert_eq!(gap_after(&columns, 1), COLUMN_GAP);
        assert_eq!(gap_after(&columns, 2), PIN_GAP);
        assert_eq!(total_gaps(&columns), 2.0 * PIN_GAP + COLUMN_GAP);

        let mut state = niri(vec![window(12, 1, Some((1, 1)), (500.0, 1000.0))]);
        state.windows.get_mut(&12).unwrap().layout.pinned = Some(niri_ipc::PinSide::Left);
        let strip = strip(&state, "eDP-1").expect("strip");
        assert_eq!(strip.columns[0].pinned, Some(niri_ipc::PinSide::Left));
    }

    #[test]
    fn scrolling_moves_between_columns() {
        use crate::modules::interaction::ModuleInteractionHandler;
        let module = MinimapModule::new(ModuleId::new("minimap"), Default::default());

        assert!(matches!(
            module.on_interaction(InteractionEvent::ScrollDown),
            Some(crate::app::Message::Minimap(MinimapEvent::FocusColumn {
                right: true
            }))
        ));
        assert!(matches!(
            module.on_interaction(InteractionEvent::ScrollUp),
            Some(crate::app::Message::Minimap(MinimapEvent::FocusColumn {
                right: false
            }))
        ));
        assert!(module.on_interaction(InteractionEvent::Clicked).is_none());
    }
}
