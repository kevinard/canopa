//! Minimap: the scrolling strip of the workspace shown on the bar's output,
//! drawn to scale. Each column is a bar of its tiled windows; the focused
//! window is accented and its column stands out. A pinned column stands apart
//! in a tinted tray with a rail along the edge of the view it is pinned to. A
//! click focuses a window, and scrolling over the strip moves focus between
//! columns. Niri owns the layout; the Shell only draws what Niri reports.
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
    /// The part of the workspace view that windows show in, from its left
    /// edge to its right one in logical pixels, when known: the output's width
    /// less the room the Shell's bars reserve at its sides.
    pub view: Option<(f64, f64)>,
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
    pub(crate) fn visible_range(&self, view: Option<(f64, f64)>) -> Option<(f32, f32)> {
        let (Some(x), Some((left, right)), true) = (self.view_x, view, self.width > 0.0) else {
            return Some((0.0, 1.0));
        };
        let start = ((left - x) / self.width).clamp(0.0, 1.0);
        let end = ((right - x) / self.width).clamp(0.0, 1.0);
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
    Some(workspace_strip(niri, workspace))
}

/// The strip of `workspace`, whether or not it is on screen.
pub(crate) fn workspace_strip(niri: &NiriState, workspace: &niri_ipc::Workspace) -> MinimapState {
    let mut tiled: Vec<(usize, usize, &niri_ipc::Window)> = Vec::new();
    let mut floating = 0;
    for window in niri.windows.values() {
        if window.workspace_id != Some(workspace.id) {
            continue;
        }
        match window.layout.pos_in_scrolling_layout {
            Some((column, tile)) => tiled.push((column, tile, window)),
            // Scratchpad windows belong to no workspace in particular; the window bar's
            // scratchpad button counts them.
            None if window.layout.in_scratchpad => {}
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

    // A bar along a side of the output covers that much of the view; Niri
    // lays windows out beside it.
    let view = workspace.output.as_deref().and_then(|output| {
        let (width, _) = niri.output_sizes.get(output)?;
        let (left, right) = niri.view_insets.get(output).copied().unwrap_or_default();
        (*width - left - right > 0.0).then_some((left, *width - right))
    });

    MinimapState {
        columns,
        workspace_height,
        floating,
        view,
    }
}

/// Spacing of a drawn strip. The bar module draws a small strip of blocks;
/// the workspace hover card draws the same layout larger, with thumbnails.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StripMetrics {
    /// Space between columns.
    pub column_gap: f32,
    /// A pinned column stands further apart from the strip it does not
    /// scroll with.
    pub pin_gap: f32,
    /// A pinned column's tiles sit this far inside its tray.
    pub pin_inset: f32,
    /// The thickness of the rail along the tray's pinned edge, standing for
    /// the edge of the view the column is fastened to.
    pub pin_rail: f32,
    /// Space between the rail and the column's tiles.
    pub pin_rail_gap: f32,
    /// Space between the tiles of a column.
    pub tile_gap: f32,
    /// The smallest extent of a column or tile.
    pub min_extent: f32,
    /// How far the on-screen frame sits outside the tiles.
    pub frame_inset: f32,
    /// Corner radius of a tile, and of the frame (plus the inset).
    pub radius: f32,
    /// Frame the part on screen even when the whole strip is on screen, so the
    /// frame always shows which windows the screen holds.
    pub always_frame: bool,
    /// The frame's line width.
    pub frame_width: f32,
}

/// The bar module's metrics.
pub(crate) const BAR_STRIP: StripMetrics = StripMetrics {
    column_gap: 2.0,
    pin_gap: 6.0,
    pin_inset: 2.0,
    pin_rail: 2.0,
    pin_rail_gap: 2.0,
    tile_gap: 1.0,
    min_extent: 2.0,
    frame_inset: 2.0,
    radius: 2.0,
    always_frame: false,
    frame_width: 1.0,
};

impl StripMetrics {
    /// The tray of a pinned column in a strip `size` pixels across, or `None`
    /// for unpinned columns. Its tiles keep their length and give up the
    /// inset across, so even a narrow column shrunk to fit keeps its tray.
    pub(crate) fn pin_tray(&self, column: &MinimapColumn, size: f32) -> Option<PinTray> {
        Some(PinTray {
            side: column.pinned?,
            tile_size: (size - 2.0 * self.pin_inset).max(self.min_extent),
        })
    }

    /// How much longer a column is drawn than its tiles: the inset and rail
    /// of a pinned column's tray.
    pub(crate) fn tray_length(&self, column: &MinimapColumn) -> f32 {
        if column.pinned.is_some() {
            2.0 * self.pin_inset + self.pin_rail + self.pin_rail_gap
        } else {
            0.0
        }
    }

    /// The space after column `index`.
    pub(crate) fn gap_after(&self, columns: &[MinimapColumn], index: usize) -> f32 {
        let pinned_boundary = columns[index].pinned == Some(niri_ipc::PinSide::Left)
            || columns
                .get(index + 1)
                .is_some_and(|next| next.pinned == Some(niri_ipc::PinSide::Right));
        if pinned_boundary {
            self.pin_gap
        } else {
            self.column_gap
        }
    }

    /// The strip's length besides its tiles: the gaps between columns and
    /// the trays of pinned ones. It stays when the strip shrinks to fit.
    fn fixed_length(&self, columns: &[MinimapColumn]) -> f32 {
        let gaps: f32 = (0..columns.len().saturating_sub(1))
            .map(|index| self.gap_after(columns, index))
            .sum();
        gaps + columns
            .iter()
            .map(|column| self.tray_length(column))
            .sum::<f32>()
    }

    /// Column lengths along the strip, in pixels: to scale with the
    /// workspace's height drawn at `size`, shrunk together to fit `max_length`.
    pub(crate) fn column_lengths(
        &self,
        state: &MinimapState,
        size: f32,
        max_length: f32,
    ) -> Vec<f32> {
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
            .map(|column| (column.width as f32 * scale).max(self.min_extent + 1.0))
            .collect();
        let fixed = self.fixed_length(&state.columns);
        let total: f32 = lengths.iter().sum::<f32>() + fixed;
        if max_length > 0.0 && total > max_length {
            let factor = ((max_length - fixed).max(0.0)) / (total - fixed);
            for length in &mut lengths {
                *length = (*length * factor).max(self.min_extent);
            }
        }
        lengths
    }

    /// The strip's length: columns, pin trays and the gaps between them.
    pub(crate) fn strip_length(&self, state: &MinimapState, lengths: &[f32]) -> f32 {
        lengths.iter().sum::<f32>() + self.fixed_length(&state.columns)
    }

    /// Tile extents across the strip: the column's tiles share `size` by height.
    pub(crate) fn tile_extents(&self, column: &MinimapColumn, size: f32) -> Vec<f32> {
        let count = column.tiles.len();
        if count == 0 {
            return Vec::new();
        }
        let available =
            (size - self.tile_gap * (count - 1) as f32).max(self.min_extent * count as f32);
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
                (available * share).max(self.min_extent)
            })
            .collect()
    }

    /// The part of the strip on screen, in strip pixels from its start, when
    /// Niri reports the view and part of the strip is off screen.
    pub(crate) fn visible_span(&self, state: &MinimapState, lengths: &[f32]) -> Option<(f32, f32)> {
        state.view?;
        if state.columns.iter().all(|column| column.view_x.is_none()) {
            return None;
        }
        let mut offset = 0.0;
        let mut span: Option<(f32, f32)> = None;
        let mut clipped = false;
        for (index, (column, length)) in state.columns.iter().zip(lengths).enumerate() {
            // A pinned column's tray goes with it.
            let length = length + self.tray_length(column);
            match column.visible_range(state.view) {
                Some((from, to)) => {
                    clipped |= from > 0.0 || to < 1.0;
                    let (start, end) = (offset + from * length, offset + to * length);
                    span = Some(span.map_or((start, end), |(first, _)| (first, end)));
                }
                None => clipped = true,
            }
            if index + 1 < state.columns.len() {
                offset += length + self.gap_after(&state.columns, index);
            }
        }
        span.filter(|_| clipped || self.always_frame)
    }
}

/// Where a tile is, for whoever draws it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TilePlace {
    /// The tile's drawn size.
    pub size: iced::Size,
    /// Its column holds the workspace's active window.
    pub active_column: bool,
    /// Part of its column is on screen.
    pub on_screen: bool,
    /// The edge of the view its column is pinned to.
    pub pinned: Option<niri_ipc::PinSide>,
    /// It is the top tile of its column.
    pub first: bool,
}

/// A pinned column's tiles sit in a tinted tray with a rail along the edge it
/// is pinned to, so it reads apart from the scrolling columns at any size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PinTray {
    /// The edge the rail runs along.
    pub side: niri_ipc::PinSide,
    /// The space the tiles share across the strip, inside the tray.
    pub tile_size: f32,
}

/// Colors a drawn strip uses besides its tiles'.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StripColors {
    /// The frame around the part on screen.
    pub frame: iced::Color,
    /// The tray behind a pinned column.
    pub pin_tray: iced::Color,
    /// The rail along a pinned column's pinned edge.
    pub pin_rail: iced::Color,
}

impl StripColors {
    /// The strip colors of the current palette: the pin tray and rail take the
    /// accent, softened enough that a focused window inside stays brightest.
    pub(crate) fn themed(frame: iced::Color, tray_alpha: f32) -> Self {
        let colors = crate::theme::palette();
        Self {
            frame,
            pin_tray: colors.accent.scale_alpha(tray_alpha),
            pin_rail: colors.accent,
        }
    }
}

/// Draw `state` as a strip `size` pixels across: columns of tiles to scale,
/// shrunk to `max_length`, with a frame around the part on screen and each
/// pinned column in its tray. Each tile is drawn by `tile`. The bar's minimap
/// and the workspace hover card share this layout and differ only in their
/// metrics and tiles.
pub(crate) fn draw_strip<'a, Message: 'a>(
    state: &MinimapState,
    metrics: StripMetrics,
    size: f32,
    max_length: f32,
    horizontal: bool,
    colors: StripColors,
    mut tile: impl FnMut(&MinimapTile, TilePlace) -> iced::Element<'a, Message>,
) -> iced::Element<'a, Message> {
    use iced::widget::{Space, column, container, row};
    let lengths = metrics.column_lengths(state, size, max_length);
    let span = metrics.visible_span(state, &lengths);
    let mut strip: Vec<iced::Element<'a, Message>> = Vec::new();
    for (index, (column_state, length)) in state
        .columns
        .iter()
        .zip(lengths.iter().copied())
        .enumerate()
    {
        if index > 0 {
            let gap = metrics.gap_after(&state.columns, index - 1);
            strip.push(if horizontal {
                Space::new().width(gap).into()
            } else {
                Space::new().height(gap).into()
            });
        }
        let on_screen = column_state.visible_range(state.view).is_some();
        let tray = metrics.pin_tray(column_state, size);
        let tile_size = tray.map_or(size, |tray| tray.tile_size);
        let tiles = column_state
            .tiles
            .iter()
            .zip(metrics.tile_extents(column_state, tile_size))
            .enumerate()
            .map(|(position, (item, extent))| {
                let (width, height) = if horizontal {
                    (length, extent)
                } else {
                    (extent, length)
                };
                tile(
                    item,
                    TilePlace {
                        size: iced::Size::new(width, height),
                        active_column: column_state.active,
                        on_screen,
                        pinned: column_state.pinned,
                        first: position == 0,
                    },
                )
            })
            .collect::<Vec<iced::Element<'a, Message>>>();
        let tiles: iced::Element<'a, Message> = if horizontal {
            column(tiles).spacing(metrics.tile_gap).into()
        } else {
            row(tiles).spacing(metrics.tile_gap).into()
        };
        let Some(tray) = tray else {
            strip.push(tiles);
            continue;
        };
        strip.push(pin_tray(
            tray, metrics, colors, horizontal, on_screen, tiles,
        ));
    }
    let strip: iced::Element<'a, Message> = if horizontal {
        row(strip).align_y(iced::Alignment::Center).into()
    } else {
        column(strip).align_x(iced::Alignment::Center).into()
    };
    let Some((start, end)) = span else {
        return strip;
    };
    // The frame around the part on screen sits `frame_inset` outside the
    // tiles; the strip is centered inside the same box.
    let inset = metrics.frame_inset;
    let across = size + 2.0 * inset;
    let radius = metrics.radius + inset;
    let frame_color = colors.frame;
    let frame = container(Space::new()).style(move |_| container::Style {
        border: iced::Border {
            color: frame_color,
            width: metrics.frame_width,
            radius: radius.into(),
        },
        ..Default::default()
    });
    let along = end - start + 2.0 * inset;
    if horizontal {
        iced::widget::stack![
            container(strip).padding([inset, inset]),
            row![Space::new().width(start), frame.width(along).height(across)],
        ]
        .into()
    } else {
        iced::widget::stack![
            container(strip).padding([inset, inset]),
            column![
                Space::new().height(start),
                frame.width(across).height(along)
            ],
        ]
        .into()
    }
}

/// A pinned column's `tiles` in their tray, with the rail on the pinned
/// side. A folded column, off screen, keeps a fainter tray.
fn pin_tray<'a, Message: 'a>(
    tray: PinTray,
    metrics: StripMetrics,
    colors: StripColors,
    horizontal: bool,
    on_screen: bool,
    tiles: iced::Element<'a, Message>,
) -> iced::Element<'a, Message> {
    use iced::widget::{Space, column, container, row};
    let fade = if on_screen { 1.0 } else { 0.5 };
    let (rail_color, tray_color) = (
        colors.pin_rail.scale_alpha(fade),
        colors.pin_tray.scale_alpha(fade),
    );
    let (rail_width, rail_height) = if horizontal {
        (metrics.pin_rail, tray.tile_size)
    } else {
        (tray.tile_size, metrics.pin_rail)
    };
    let rail: iced::Element<'a, Message> =
        container(Space::new().width(rail_width).height(rail_height))
            .style(move |_| container::Style {
                background: Some(rail_color.into()),
                border: iced::border::rounded(metrics.pin_rail / 2.0),
                ..Default::default()
            })
            .into();
    let parts = match tray.side {
        niri_ipc::PinSide::Left => [rail, tiles],
        niri_ipc::PinSide::Right => [tiles, rail],
    };
    let content: iced::Element<'a, Message> = if horizontal {
        row(parts).spacing(metrics.pin_rail_gap).into()
    } else {
        column(parts).spacing(metrics.pin_rail_gap).into()
    };
    container(content)
        .padding(metrics.pin_inset)
        .style(move |_| container::Style {
            background: Some(tray_color.into()),
            border: iced::border::rounded(metrics.radius + metrics.pin_inset),
            ..Default::default()
        })
        .into()
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
        use iced::widget::{Space, container, mouse_area};
        let size = f32::from(self.config.size);
        let colors = crate::theme::palette();

        if state.columns.is_empty() {
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

        let strip_colors = StripColors::themed(colors.text.scale_alpha(0.75), 0.3);
        draw_strip(
            state,
            BAR_STRIP,
            size,
            self.config.max_length as f32,
            horizontal,
            strip_colors,
            |tile, place| {
                let color = if tile.urgent {
                    colors.warning
                } else if tile.focused {
                    colors.accent
                } else if place.active_column {
                    iced::Color {
                        a: 0.7,
                        ..colors.text
                    }
                } else if place.on_screen {
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
                let block = container(
                    Space::new()
                        .width(place.size.width)
                        .height(place.size.height),
                )
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(color)),
                    border: iced::border::rounded(BAR_STRIP.radius),
                    ..Default::default()
                });
                mouse_area(block)
                    .on_press(crate::app::Message::Minimap(MinimapEvent::FocusWindow(
                        tile.id,
                    )))
                    .interaction(iced::mouse::Interaction::Pointer)
                    .into()
            },
        )
    }
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
                in_scratchpad: false,
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
    fn the_view_leaves_out_the_bars_at_its_sides() {
        let mut state = niri(vec![window(12, 1, Some((1, 1)), (960.0, 500.0))]);
        state.output_sizes.insert("eDP-1".into(), (1920.0, 1080.0));
        assert_eq!(
            strip(&state, "eDP-1").expect("strip").view,
            Some((0.0, 1920.0))
        );
        state.view_insets.insert("eDP-1".into(), (76.0, 0.0));
        assert_eq!(
            strip(&state, "eDP-1").expect("strip").view,
            Some((76.0, 1920.0))
        );
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
            view: None,
        };

        assert_eq!(BAR_STRIP.column_lengths(&state, 20.0, 0.0), [20.0, 10.0]);
        let fitted = BAR_STRIP.column_lengths(&state, 20.0, 17.0);
        assert_eq!(fitted, [10.0, 5.0]);
        assert_eq!(fitted.iter().sum::<f32>() + BAR_STRIP.column_gap, 17.0);
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

        assert_eq!(BAR_STRIP.tile_extents(&column, 19.0), [6.0, 12.0]);
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
            view: Some((0.0, 1000.0)),
        };
        let lengths = [10.0, 10.0, 20.0, 10.0];
        assert_eq!(state.columns[0].visible_range(state.view), None);
        assert_eq!(state.columns[2].visible_range(state.view), Some((0.0, 0.5)));
        assert_eq!(BAR_STRIP.visible_span(&state, &lengths), Some((12.0, 34.0)));
        // A column scrolled under a bar at the side of the view is off screen.
        let beside_bar = MinimapColumn {
            width: 500.0,
            view_x: Some(-424.0),
            ..Default::default()
        };
        assert_eq!(beside_bar.visible_range(Some((76.0, 1000.0))), None);
        assert!(beside_bar.visible_range(Some((0.0, 1000.0))).is_some());

        // Everything on screen, or no positions from Niri: no frame on the bar.
        let all = MinimapState {
            columns: vec![column(0.0), column(500.0)],
            ..state.clone()
        };
        assert_eq!(BAR_STRIP.visible_span(&all, &lengths), None);
        // A strip that always frames the screen still frames every column on it.
        let framed = StripMetrics {
            always_frame: true,
            ..BAR_STRIP
        };
        assert_eq!(framed.visible_span(&all, &lengths), Some((0.0, 22.0)));
        let unknown = MinimapState {
            columns: vec![MinimapColumn {
                width: 500.0,
                ..Default::default()
            }],
            ..state
        };
        assert_eq!(BAR_STRIP.visible_span(&unknown, &lengths), None);
    }

    #[test]
    fn pinned_columns_stand_apart_with_a_rail_on_their_edge() {
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
        assert_eq!(BAR_STRIP.gap_after(&columns, 0), BAR_STRIP.pin_gap);
        assert_eq!(BAR_STRIP.gap_after(&columns, 1), BAR_STRIP.column_gap);
        assert_eq!(BAR_STRIP.gap_after(&columns, 2), BAR_STRIP.pin_gap);

        // Pinned columns hold their tiles in a tray, inset across, with a
        // rail on their pinned side.
        let inset = 2.0 * BAR_STRIP.pin_inset;
        let tray = inset + BAR_STRIP.pin_rail + BAR_STRIP.pin_rail_gap;
        assert_eq!(
            BAR_STRIP.pin_tray(&columns[0], 18.0),
            Some(PinTray {
                side: niri_ipc::PinSide::Left,
                tile_size: 18.0 - inset,
            })
        );
        assert_eq!(
            BAR_STRIP.pin_tray(&columns[3], 18.0).map(|tray| tray.side),
            Some(niri_ipc::PinSide::Right)
        );
        assert_eq!(BAR_STRIP.pin_tray(&columns[1], 18.0), None);
        // Trays and gaps keep their length while a long strip shrinks, so a
        // narrow pinned column still shows its tray.
        let fixed = 2.0 * BAR_STRIP.pin_gap + BAR_STRIP.column_gap + 2.0 * tray;
        assert_eq!(BAR_STRIP.fixed_length(&columns), fixed);
        let state = MinimapState {
            columns: columns.to_vec(),
            workspace_height: 1000.0,
            ..Default::default()
        };
        let lengths = BAR_STRIP.column_lengths(&state, 20.0, 60.0);
        assert_eq!(BAR_STRIP.strip_length(&state, &lengths), 60.0);
        assert_eq!(lengths[0], (60.0 - fixed) / 4.0);

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
